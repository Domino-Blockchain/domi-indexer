use {
    crate::postgres_client::{ParallelPostgresClient, PostgresClientBuilder},
    bs58,
    domichain_geyser_plugin_interface::geyser_plugin_interface::{
        GeyserPlugin, GeyserPluginError, ReplicaAccountInfoVersions, ReplicaBlockInfoVersions,
        ReplicaTransactionInfoVersions, Result, SlotStatus,
    },
    domichain_measure::measure::Measure,
    domichain_metrics::*,
    domichain_runtime::contains::Contains,
    log::*,
    serde_derive::{Deserialize, Serialize},
    serde_json,
    std::{fs::File, io::Read},
    thiserror::Error,
};

#[derive(Default)]
pub struct GeyserPluginPostgres {
    client: Option<ParallelPostgresClient>,
    config: GeyserPluginPostgresConfig,
}

impl std::fmt::Debug for GeyserPluginPostgres {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

/// The Configuration for the PostgreSQL plugin
#[derive(Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GeyserPluginPostgresConfig {
    /// The host name or IP of the PostgreSQL server
    pub host: Option<String>,

    /// The user name of the PostgreSQL server.
    pub user: Option<String>,

    /// The port number of the PostgreSQL database, the default is 5432
    pub port: Option<u16>,

    /// The connection string of PostgreSQL database, if this is set
    /// `host`, `user` and `port` will be ignored.
    pub connection_str: Option<String>,

    /// Controls the number of threads establishing connections to
    /// the PostgreSQL server. The default is 10.
    pub threads: Option<usize>,

    /// Controls the batch size when bulk loading accounts.
    /// The default is 10.
    pub batch_size: Option<usize>,

    /// Controls whether to panic the validator in case of errors
    /// writing to PostgreSQL server. The default is false
    pub panic_on_db_errors: Option<bool>,

    /// Controls whether to use SSL based connection to the database server.
    /// The default is false
    pub use_ssl: Option<bool>,

    /// Specify the path to PostgreSQL server's certificate file
    pub server_ca: Option<String>,

    /// Specify the path to the local client's certificate file
    pub client_cert: Option<String>,

    /// Specify the path to the local client's private PEM key file.
    pub client_key: Option<String>,

    /// Program ID of the inscription program
    pub program_id: Option<String>,
}

#[derive(Error, Debug)]
pub enum GeyserPluginPostgresError {
    #[error("Error connecting to the backend data store. Error message: ({msg})")]
    DataStoreConnectionError { msg: String },

    #[error("Error preparing data store schema. Error message: ({msg})")]
    DataSchemaError { msg: String },

    #[error("Error preparing data store schema. Error message: ({msg})")]
    ConfigurationError { msg: String },

    #[error("Replica account V0.0.1 not supported anymore")]
    ReplicaAccountV001NotSupported,
}

impl GeyserPlugin for GeyserPluginPostgres {
    fn name(&self) -> &'static str {
        "GeyserPluginPostgres"
    }

    fn on_load(&mut self, config_file: &str) -> Result<()> {
        domichain_logger::setup_with_default("info");
        info!(
            "Loading plugin {:?} from config_file {:?}",
            self.name(),
            config_file
        );
        let mut file = File::open(config_file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        self.config = serde_json::from_str(&contents).map_err(|err| {
            GeyserPluginError::ConfigFileReadError {
                msg: format!(
                    "The config file is not in the JSON format expected: {:?}",
                    err
                ),
            }
        })?;

        let client = PostgresClientBuilder::build_pararallel_postgres_client(&self.config)?;
        self.client = Some(client);

        Ok(())
    }

    fn on_unload(&mut self) {
        info!("Unloading plugin: {:?}", self.name());

        match &mut self.client {
            None => {}
            Some(client) => {
                client.join().unwrap();
            }
        }
    }

    fn update_account(
        &self,
        _account: ReplicaAccountInfoVersions,
        _slot: u64,
        _is_startup: bool,
    ) -> Result<()> {
        Ok(())
    }

    fn update_slot_status(
        &self,
        slot: u64,
        _parent: Option<u64>,
        _status: SlotStatus,
    ) -> Result<()> {
        Ok(())
    }

    fn notify_end_of_startup(&self) -> Result<()> {
        Ok(())
    }

    fn notify_transaction(
        &self,
        transaction_info: ReplicaTransactionInfoVersions,
        slot: u64,
    ) -> Result<()> {
        match &self.client {
            None => {
                return Err(GeyserPluginError::Custom(Box::new(
                    GeyserPluginPostgresError::DataStoreConnectionError {
                        msg: "There is no connection to the PostgreSQL database.".to_string(),
                    },
                )));
            }
            Some(client) => match transaction_info {
                ReplicaTransactionInfoVersions::V0_0_2(transaction_info) => {
                    if transaction_info.transaction_status_meta.status.is_err() {
                        return Ok(());
                    }

                    if !transaction_info
                        .transaction
                        .message()
                        .account_keys()
                        .iter()
                        .any(|key| key == &mpl_inscription_program::ID)
                    {
                        return Ok(());
                    }

                    let result = client.log_inscription_info(transaction_info, slot);

                    if let Err(err) = result {
                        return Err(GeyserPluginError::SlotStatusUpdateError{
                                msg: format!("Failed to persist the transaction info to the PostgreSQL database. Error: {:?}", err)
                            });
                    }
                }
                _ => {
                    return Err(GeyserPluginError::SlotStatusUpdateError{
                        msg: "Failed to persist the transaction info to the PostgreSQL database. Unsupported format.".to_string()
                    });
                }
            },
        }

        Ok(())
    }

    fn notify_block_metadata(&self, _block_info: ReplicaBlockInfoVersions) -> Result<()> {
        Ok(())
    }

    fn account_data_notifications_enabled(&self) -> bool {
        false
    }

    fn transaction_notifications_enabled(&self) -> bool {
        true
    }
}

impl GeyserPluginPostgres {
    pub fn new() -> Self {
        Self::default()
    }
}

#[no_mangle]
#[allow(improper_ctypes_definitions)]
/// # Safety
///
/// This function returns the GeyserPluginPostgres pointer as trait GeyserPlugin.
pub unsafe extern "C" fn _create_plugin() -> *mut dyn GeyserPlugin {
    let plugin = GeyserPluginPostgres::new();
    let plugin: Box<dyn GeyserPlugin> = Box::new(plugin);
    Box::into_raw(plugin)
}
