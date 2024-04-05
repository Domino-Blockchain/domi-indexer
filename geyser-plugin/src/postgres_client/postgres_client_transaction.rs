/// Module responsible for handling persisting transaction data to the PostgreSQL
/// database.
use {
    crate::{
        geyser_plugin_postgres::{GeyserPluginPostgresConfig, GeyserPluginPostgresError},
        postgres_client::{DbWorkItem, ParallelPostgresClient, SimplePostgresClient},
    },
    borsh::BorshDeserialize,
    chrono::Utc,
    domichain_geyser_plugin_interface::geyser_plugin_interface::{
        GeyserPluginError, ReplicaTransactionInfoV2,
    },
    domichain_runtime::{bank::RewardType, transaction_batch},
    domichain_sdk::{
        account_info::AccountInfo,
        instruction::CompiledInstruction,
        message::{
            v0::{self, LoadedAddresses, MessageAddressTableLookup},
            Message, MessageHeader, SanitizedMessage,
        },
        pubkey::Pubkey,
        transaction::TransactionError,
    },
    domichain_transaction_status::{
        InnerInstructions, Reward, TransactionStatusMeta, TransactionTokenBalance,
    },
    log::*,
    mpl_inscription_program::instruction::{
        accounts::WriteDataAccounts, MplInscriptionInstruction,
    },
    postgres::{Client, Statement},
    postgres_types::{FromSql, ToSql},
    std::sync::atomic::Ordering,
};

pub struct DbInscription {
    pub slot: i64,
    pub signature: String,
    pub account: String,
    pub metadata_account: String,
    pub authority: String,
    pub data: Vec<u8>,
    /// This can be used to tell the order of transaction within a block
    /// Given a slot, the transaction with a smaller write_version appears
    /// before transactions with higher write_versions in a shred.
    pub write_version: i64,
}

pub struct LogInscriptionRequest {
    pub inscription_info: DbInscription,
}

impl SimplePostgresClient {
    pub(crate) fn build_inscription_info_upsert_statement(
        client: &mut Client,
        config: &GeyserPluginPostgresConfig,
    ) -> Result<Statement, GeyserPluginError> {
        let stmt = "INSERT INTO inscriptions AS insc (slot, signature, account, metadata_account, \
                authority, data, write_version, updated_on) \
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (account) DO UPDATE SET \
                account=excluded.account, \
                metadata_account=excluded.metadata_account, \
                authority=excluded.authority, \
                data=excluded.data, \
                write_version=excluded.write_version, \
                updated_on=excluded.updated_on";

        let stmt = client.prepare(stmt);

        match stmt {
            Err(err) => {
                Err(GeyserPluginError::Custom(Box::new(GeyserPluginPostgresError::DataSchemaError {
                    msg: format!(
                        "Error in preparing for the transaction update PostgreSQL database: ({}) host: {:?} user: {:?} config: {:?}",
                        err, config.host, config.user, config
                    ),
                })))
            }
            Ok(stmt) => Ok(stmt),
        }
    }

    pub(crate) fn log_inscription_impl(
        &mut self,
        inscription_log_info: LogInscriptionRequest,
    ) -> Result<(), GeyserPluginError> {
        let client = self.client.get_mut().unwrap();
        let statement = &client.update_inscription_log_stmt;
        let client = &mut client.client;
        let updated_on = Utc::now().naive_utc();

        let inscription_info = inscription_log_info.inscription_info;
        let result = client.query(
            statement,
            &[
                &inscription_info.slot,
                &inscription_info.signature,
                &inscription_info.account,
                &inscription_info.metadata_account,
                &inscription_info.authority,
                &inscription_info.data,
                &inscription_info.write_version,
                &updated_on,
            ],
        );

        if let Err(err) = result {
            let msg = format!(
                "Failed to persist the update of transaction info to the PostgreSQL database. Error: {:?}",
                err
            );
            error!("{}", msg);
            return Err(GeyserPluginError::AccountsUpdateError { msg });
        }

        Ok(())
    }
}

impl ParallelPostgresClient {
    pub fn log_inscription_info(
        &self,
        transaction_info: &ReplicaTransactionInfoV2,
        slot: u64,
    ) -> Result<(), GeyserPluginError> {
        self.transaction_write_version
            .fetch_add(1, Ordering::Relaxed);

        let instructions = match transaction_info.transaction.message() {
            SanitizedMessage::Legacy(message) => &message.message.instructions,
            SanitizedMessage::V0(message) => &message.message.instructions,
        };

        let write_data_tuple = instructions.into_iter().find_map(|instruction| {
            match MplInscriptionInstruction::try_from_slice(&instruction.data) {
                Ok(MplInscriptionInstruction::WriteData(args)) => Some((instruction.clone(), args)),
                Ok(_) | Err(_) => None,
            }
        });

        if let Some((write_instruction, write_args)) = write_data_tuple {
            let account_keys = match transaction_info.transaction.message() {
                SanitizedMessage::Legacy(message) => message.account_keys(),
                SanitizedMessage::V0(message) => message.account_keys(),
            };

            let instruction_accounts = write_instruction
                .accounts
                .into_iter()
                .filter_map(|index| account_keys.get(index.into()))
                .collect::<Vec<_>>();

            fn get_account_pubkey_slice<'a>(
                instruction_accounts: &'a Vec<&'a Pubkey>,
                index: usize,
            ) -> String {
                bs58::encode(instruction_accounts.get(index).unwrap().as_ref()).into_string()
            }

            let inscription_info = DbInscription {
                slot: slot as i64,
                account: get_account_pubkey_slice(&instruction_accounts, 0),
                metadata_account: get_account_pubkey_slice(&instruction_accounts, 1),
                authority: get_account_pubkey_slice(&instruction_accounts, 2),
                data: write_args.value,
                write_version: self.transaction_write_version.load(Ordering::Relaxed) as i64,
                signature: bs58::encode(transaction_info.signature.as_ref()).into_string(),
            };

            let wrk_item =
                DbWorkItem::LogInscription(Box::new(LogInscriptionRequest { inscription_info }));

            if let Err(err) = self.sender.send(wrk_item) {
                return Err(GeyserPluginError::SlotStatusUpdateError {
                    msg: format!("Failed to update the transaction, error: {:?}", err),
                });
            }
        }

        Ok(())
    }
}
