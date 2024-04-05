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
    pub mint_account: Option<String>,
    pub metadata_account: String,
    pub authority: String,
}

pub struct LogInscriptionRequest {
    pub inscription_info: DbInscription,
}

impl SimplePostgresClient {
    pub(crate) fn build_inscription_info_upsert_statement(
        client: &mut Client,
        config: &GeyserPluginPostgresConfig,
    ) -> Result<Statement, GeyserPluginError> {
        let stmt = "INSERT INTO inscriptions AS insc (slot, signature, account, mint_account, \
                metadata_account, authority, updated_on) \
            VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (account) DO UPDATE SET \
                account=excluded.account, \
                mint_account=excluded.mint_account, \
                metadata_account=excluded.metadata_account, \
                authority=excluded.authority, \
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
                &inscription_info.mint_account,
                &inscription_info.metadata_account,
                &inscription_info.authority,
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
        let instructions = match transaction_info.transaction.message() {
            SanitizedMessage::Legacy(message) => &message.message.instructions,
            SanitizedMessage::V0(message) => &message.message.instructions,
        };

        let inscription_instructions = instructions
            .into_iter()
            .filter_map(|compiled| {
                MplInscriptionInstruction::try_from_slice(&compiled.data)
                    .ok()
                    .and_then(|parsed| Some((compiled.clone(), parsed)))
            })
            .collect::<Vec<_>>();

        if inscription_instructions.is_empty() {
            return Ok(());
        }

        let account_keys = match transaction_info.transaction.message() {
            SanitizedMessage::Legacy(message) => message.account_keys(),
            SanitizedMessage::V0(message) => message.account_keys(),
        };

        fn get_account_pubkey_slice<'a>(
            instruction_accounts: &'a Vec<&'a Pubkey>,
            index: usize,
        ) -> String {
            bs58::encode(instruction_accounts.get(index).unwrap().as_ref()).into_string()
        }

        let mut inscription_info: Option<DbInscription> = None;

        for (compiled_instruction, inscription_instruction) in inscription_instructions {
            match inscription_instruction {
                MplInscriptionInstruction::Initialize => {
                    let instruction_accounts = compiled_instruction
                        .accounts
                        .into_iter()
                        .filter_map(|index| account_keys.get(index.into()))
                        .collect::<Vec<_>>();

                    inscription_info = Some(DbInscription {
                        slot: slot as i64,
                        account: get_account_pubkey_slice(&instruction_accounts, 0),
                        metadata_account: get_account_pubkey_slice(&instruction_accounts, 1),
                        mint_account: None,
                        authority: get_account_pubkey_slice(&instruction_accounts, 3),
                        signature: bs58::encode(transaction_info.signature.as_ref()).into_string(),
                    });
                }

                MplInscriptionInstruction::InitializeFromMint => {
                    let instruction_accounts = compiled_instruction
                        .accounts
                        .into_iter()
                        .filter_map(|index| account_keys.get(index.into()))
                        .collect::<Vec<_>>();

                    inscription_info = Some(DbInscription {
                        slot: slot as i64,
                        account: get_account_pubkey_slice(&instruction_accounts, 0),
                        metadata_account: get_account_pubkey_slice(&instruction_accounts, 1),
                        mint_account: Some(get_account_pubkey_slice(&instruction_accounts, 2)),
                        authority: get_account_pubkey_slice(&instruction_accounts, 5),
                        signature: bs58::encode(transaction_info.signature.as_ref()).into_string(),
                    });
                }

                _ => {
                    // do nothing
                }
            }
        }

        if let Some(inscription_info) = inscription_info {
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
