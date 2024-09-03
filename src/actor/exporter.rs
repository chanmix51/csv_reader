//! # Account Exporter Actor
//!
//! This module provides the implementation of the Account Exporter Actor.

use std::{io::Write, sync::Arc};

use log::debug;

use crate::{service::AccountManager, Result};

/// The account exporter actor.
pub struct AccountExporter {
    /// The account manager service.
    account_manager: Arc<AccountManager>,

    /// A Write interface to export the CSV to
    writer: Box<dyn Write + Sync + Send>,
}

impl AccountExporter {
    /// Create a new account exporter actor.
    pub fn new(account_manager: Arc<AccountManager>, writer: Box<dyn Write + Sync + Send>) -> Self {
        Self {
            account_manager,
            writer,
        }
    }

    /// Run the account exporter actor.
    /// The actor will export the accounts to a CSV file.
    pub fn run(self) -> Result<()> {
        debug!("Account Exporter Actor started");

        let accounts = self.account_manager.get_accounts();

        let mut writer = csv::Writer::from_writer(self.writer);
        for account in accounts {
            writer.serialize(account)?;
        }

        writer.flush()?;

        debug!("Account Exporter Actor stopped");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use rust_decimal::Decimal;

    use super::*;
    use crate::{
        adapter::InMemoryAccountStorage,
        model::{TransactionKind, TransactionOrder},
    };

    #[test]
    fn test_account_exporter_actor() {
        let account_manager = Arc::new(AccountManager::new(InMemoryAccountStorage::default()));
        account_manager
            .process_order(TransactionOrder {
                tx_id: 1,
                client_id: 1,
                kind: TransactionKind::Deposit(Decimal::ONE_HUNDRED),
            })
            .unwrap();
        let writer = Cursor::new(Vec::new());
        let account_exporter = AccountExporter::new(account_manager, Box::new(writer));

        account_exporter.run().unwrap();
    }
}
