use std::collections::{HashMap, HashSet};

use anyhow::anyhow;

use crate::model::{Account, ClientId, Transaction, TxId};
use crate::Result;

/// Account storage trait.
///
/// This trait defines the operations that can be performed on an account
/// storage.  It must raise an error only if the operation leads to a non
/// consistent state or if there are IO errors.
pub trait AccountStorage {
    /// Get an account by its client id.
    fn get_account(&self, client_id: &ClientId) -> Option<Account>;

    /// Get a transaction by its identifier.
    fn get_transaction(&self, tx_id: &TxId) -> Option<Transaction>;

    /// Check if a transaction is disputed.
    fn is_disputed(&self, tx_id: &TxId) -> Option<bool>;

    /// Add or update an account.
    fn store_account(&mut self, account: Account) -> Result<Account>;

    /// Store a new transaction.
    /// Fails if the transaction already exists.
    fn store_transaction(&mut self, transaction: Transaction) -> Result<Transaction>;

    /// Set a transaction as disputed or not.
    /// Fails if the transaction does not exist.
    fn set_disputed(&mut self, tx_id: TxId, disputed: bool) -> Result<()>;
}

/// A simple in-memory account storage.
#[derive(Debug, Default)]
pub struct InMemoryAccountStorage {
    accounts: HashMap<ClientId, Account>,
    transactions: HashMap<TxId, Transaction>,
    disputed: HashSet<TxId>,
}

impl AccountStorage for InMemoryAccountStorage {
    fn get_account(&self, client_id: &ClientId) -> Option<Account> {
        self.accounts.get(client_id).cloned()
    }

    fn get_transaction(&self, tx_id: &TxId) -> Option<Transaction> {
        self.transactions.get(tx_id).cloned()
    }

    fn is_disputed(&self, tx_id: &TxId) -> Option<bool> {
        self.transactions
            .get(tx_id)
            .map(|_| self.disputed.contains(tx_id))
    }

    fn store_account(&mut self, account: Account) -> Result<Account> {
        self.accounts.insert(account.client_id, account.clone());

        Ok(account)
    }

    fn store_transaction(&mut self, transaction: Transaction) -> Result<Transaction> {
        if self.transactions.contains_key(&transaction.tx_id) {
            return Err(anyhow!("Transaction {} already exists", transaction.tx_id));
        }
        self.transactions
            .insert(transaction.tx_id, transaction.clone());

        Ok(transaction)
    }

    fn set_disputed(&mut self, tx_id: TxId, disputed: bool) -> Result<()> {
        let _ = self
            .transactions
            .get(&tx_id)
            .ok_or_else(|| anyhow!("Transaction {} does not exist", tx_id))?;

        if disputed {
            self.disputed.insert(tx_id);
        } else {
            self.disputed.remove(&tx_id);
        }

        Ok(())
    }
}

#[cfg(test)]
mod in_memory_storage_tests {
    use rust_decimal_macros::dec;

    use crate::model::{TransactionKind, TransactionOrder};

    use super::*;

    #[test]
    fn test_get_account_exists() {
        let mut storage = InMemoryAccountStorage::default();
        let account = Account::new(1);
        storage.accounts.insert(1, account.clone());

        assert_eq!(storage.get_account(&1), Some(account));
    }

    #[test]
    fn test_get_account_not_exists() {
        let storage = InMemoryAccountStorage::default();

        assert_eq!(storage.get_account(&1), None);
    }

    #[test]
    fn test_get_transaction_exists() {
        let mut storage = InMemoryAccountStorage::default();
        let transaction: Transaction = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(dec!(1)),
        }
        .into();
        storage.transactions.insert(1, transaction.clone());

        assert_eq!(storage.get_transaction(&1), Some(transaction));
    }

    #[test]
    fn test_get_transaction_not_exists() {
        let storage = InMemoryAccountStorage::default();

        assert_eq!(storage.get_transaction(&1), None);
    }

    #[test]
    fn test_set_disputed() {
        let mut storage = InMemoryAccountStorage::default();

        // Non existing transaction returns None
        assert!(storage.is_disputed(&1).is_none());

        let transaction: Transaction = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(dec!(1)),
        }
        .into();
        storage.transactions.insert(1, transaction.clone());

        // By default, transactions are not disputed
        assert!(!storage.is_disputed(&1).unwrap());

        storage.set_disputed(1, true).unwrap();

        // Transaction is now disputed
        assert!(storage.is_disputed(&1).unwrap());

        storage.set_disputed(1, true).unwrap();

        // Transaction is still disputed
        assert!(storage.is_disputed(&1).unwrap());

        storage.set_disputed(1, false).unwrap();

        // Transaction is not disputed anymore
        assert!(!storage.is_disputed(&1).unwrap());
    }

    #[test]
    fn test_set_disputed_non_existing_transaction() {
        let mut storage = InMemoryAccountStorage::default();
        let error = storage.set_disputed(1, true).unwrap_err();

        assert_eq!(error.to_string(), "Transaction 1 does not exist");
    }

    #[test]
    fn test_store_account() {
        let mut storage = InMemoryAccountStorage::default();
        let account = Account::new(1);
        let account = storage.store_account(account).unwrap();

        assert_eq!(storage.accounts.get(&1), Some(&account));
    }

    #[test]
    fn test_store_transaction() {
        let mut storage = InMemoryAccountStorage::default();
        let transaction: Transaction = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(dec!(1)),
        }
        .into();
        let transaction = storage.store_transaction(transaction).unwrap();

        assert_eq!(storage.transactions.get(&1), Some(&transaction));
    }

    #[test]
    fn test_store_transaction_already_exists() {
        let mut storage = InMemoryAccountStorage::default();
        let transaction: Transaction = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(dec!(1)),
        }
        .into();
        let _ = storage.store_transaction(transaction.clone()).unwrap();
        let error = storage.store_transaction(transaction).unwrap_err();

        assert_eq!(error.to_string(), "Transaction 1 already exists");
    }
}
