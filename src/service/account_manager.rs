use std::sync::RwLock;

use anyhow::{anyhow, bail};
use rust_decimal::Decimal;

use crate::adapter::AccountStorage;
use crate::model::{Account, ClientId, Transaction, TransactionKind, TransactionOrder, TxId};
use crate::Result;

/// Transaction related errors.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    /// The transaction idenfier is already in use.
    #[error("Transaction id='{0}' already in use.")]
    DuplicateTransactionId(TxId),

    /// Related transaction not found, either it does not exist or it was not a
    /// disputable transaction.
    #[error("Related transaction id='{0}' not found.")]
    RelatedTransactionNotFound(TxId),

    /// The related transaction is not disputed.
    #[error("Transaction id='{0}' is not disputed.")]
    NonDisputedTransaction(TxId),

    /// The related transaction is already disputed.
    #[error("Transaction id='{0}' is already disputed")]
    AlreadyDisputedTransaction(TxId),

    /// The related transaction is not disputable.
    #[error("Related transaction id='{0}' is not disputable (must be a deposit).")]
    RelatedTransactionNotDisputable(TxId),
}

/// The [AccountManager] is responsible for managing the accounts and
/// transactions of the system.  It turns [TransactionOrder]s into
/// [Transaction]s and applies them to the accounts.
///
/// This service can be shared amongst multiple actors hence muliple threads.
/// This means it will be stored in an `Arc so its internal state must use
/// interior mutability.
/// For now we will use a simple hash map to store the accounts and transactions
/// but adapters can be used to store the data in a database.
pub struct AccountManager {
    /// Storing the internal state in one place protected by a read-write lock.
    /// This prevent some actors to read inconsistent data.
    store: RwLock<Box<dyn AccountStorage + Sync + Send>>,
}

impl AccountManager {
    /// Create a new account manager.
    pub fn new(storage: impl AccountStorage + Sync + Send + 'static) -> Self {
        Self {
            store: RwLock::new(Box::new(storage)),
        }
    }

    /// Try to process the given order and return the resulting transaction.
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use rust_decimal::Decimal;
    /// use rust_decimal_macros::dec;
    ///
    /// use csv_reader::model::{TransactionOrder, TransactionKind};
    /// use csv_reader::adapter::InMemoryAccountStorage;
    /// use csv_reader::service::AccountManager;
    ///
    /// let manager = Arc::new(AccountManager::new(InMemoryAccountStorage::default()));
    /// let transaction = manager.process_order(TransactionOrder { tx_id: 1, client_id: 1, kind: TransactionKind::Deposit(Decimal::ONE_HUNDRED) }).unwrap();
    ///
    /// assert_eq!(transaction.tx_id, 1);
    /// let account = manager.get_account(1).unwrap();
    ///
    /// assert_eq!(account.available, Decimal::ONE_HUNDRED);
    ///
    /// let _tx = manager.process_order(TransactionOrder { tx_id: 2, client_id: 1, kind: TransactionKind::Withdrawal(dec!(30)) }).unwrap();
    /// let account = manager.get_account(1).unwrap();
    ///
    /// assert_eq!(account.available, dec!(70));
    ///
    /// let _tx = manager.process_order(TransactionOrder { tx_id: 3, client_id: 2, kind: TransactionKind::Dispute(1) }).unwrap();
    /// let account = manager.get_account(1).unwrap();
    ///
    /// assert_eq!(account.available, dec!(-30));
    ///
    /// let _tx = manager.process_order(TransactionOrder { tx_id: 4, client_id: 1, kind: TransactionKind::Deposit(Decimal::ONE_HUNDRED) }).unwrap();
    /// let _tx = manager.process_order(TransactionOrder { tx_id: 5, client_id: 2, kind: TransactionKind::Resolve(1) }).unwrap();
    /// let account = manager.get_account(1).unwrap();
    ///
    /// assert_eq!(account.available, dec!(170));
    ///
    /// let _tx = manager.process_order(TransactionOrder { tx_id: 6, client_id: 2, kind: TransactionKind::Dispute(4) }).unwrap();
    /// let _tx = manager.process_order(TransactionOrder { tx_id: 7, client_id: 2, kind: TransactionKind::ChargeBack(4) }).unwrap();
    /// let account = manager.get_account(1).unwrap();
    ///
    /// assert_eq!(account.available, dec!(70));
    /// assert!(account.locked);
    /// ```
    ///
    pub fn process_order(&self, order: TransactionOrder) -> Result<Transaction> {
        let transaction: Transaction = order.into();

        let transaction = match transaction.kind {
            TransactionKind::Deposit(amount) => self.process_deposit(transaction, amount)?,
            TransactionKind::Withdrawal(amount) => self.process_withdrawal(transaction, amount)?,
            TransactionKind::Dispute(tx_id) => self.process_dispute(transaction, tx_id)?,
            TransactionKind::Resolve(tx_id) => self.process_resolve(transaction, tx_id)?,
            TransactionKind::ChargeBack(tx_id) => self.process_chargeback(transaction, tx_id)?,
        };

        Ok(transaction)
    }

    /// Get the account for the given client identifier.
    ///
    /// ```
    /// use rust_decimal::Decimal;
    ///
    /// use csv_reader::adapter::InMemoryAccountStorage;
    /// use csv_reader::model::{Account, ClientId, TransactionKind, TransactionOrder};
    /// use csv_reader::service::AccountManager;
    ///
    /// let manager = AccountManager::new(InMemoryAccountStorage::default());
    ///
    /// // If the account does not exist, None is returned.
    /// assert!(manager.get_account(1).is_none());
    ///
    /// // If the account exists, it is returned.
    /// let order = TransactionOrder {
    ///     tx_id: 1,
    ///     client_id: 1,
    ///     kind: TransactionKind::Deposit(Decimal::ONE),
    /// };
    /// let _transaction = manager.process_order(order).unwrap();
    /// let account = manager.get_account(1).unwrap();
    /// assert_eq!(account.client_id, 1);
    /// assert_eq!(account.available, Decimal::ONE);
    ///
    /// ```
    pub fn get_account(&self, client_id: ClientId) -> Option<Account> {
        // If the lock returns an error, it means that a thread panicked while
        // holding the lock so this thread should panic as well.
        self.store.read().unwrap().get_account(&client_id)
    }

    /// Export the accounts.
    pub fn get_accounts(&self) -> Vec<Account> {
        self.store.read().unwrap().get_accounts()
    }

    /// Get the disputable transaction for the given transaction identifier.
    fn get_disputable_transaction(&self, tx_id: TxId) -> Option<Transaction> {
        self.store.read().unwrap().get_transaction(&tx_id)
    }

    /// Process a deposit order.
    fn process_deposit(&self, transaction: Transaction, amount: Decimal) -> Result<Transaction> {
        // if the transaction id is already in use, return an error.
        if self.get_disputable_transaction(transaction.tx_id).is_some() {
            return Err(anyhow::anyhow!(TransactionError::DuplicateTransactionId(
                transaction.tx_id
            )));
        }

        // prefer to panic if the lock is poisoned ↓.
        let mut guard = self.store.write().unwrap();
        let mut account = guard
            .get_account(&transaction.client_id)
            .unwrap_or(Account::new(transaction.client_id));
        account.deposit(amount)?;
        guard.store_account(account)?;

        guard.store_transaction(transaction)
    }

    /// Process a withdrawal order.
    fn process_withdrawal(&self, transaction: Transaction, amount: Decimal) -> Result<Transaction> {
        // if the transaction id is already in use, return an error.
        if self.get_disputable_transaction(transaction.tx_id).is_some() {
            return Err(anyhow::anyhow!(TransactionError::DuplicateTransactionId(
                transaction.tx_id
            )));
        }

        let mut guard = self.store.write().unwrap();
        let mut account = guard
            .get_account(&transaction.client_id)
            .unwrap_or(Account::new(transaction.client_id));
        account.withdraw(amount)?;
        guard.store_account(account)?;

        guard.store_transaction(transaction)
    }

    /// Process a dispute order.
    fn process_dispute(
        &self,
        transaction: Transaction,
        related_transaction_id: TxId,
    ) -> Result<Transaction> {
        let mut guard = self.store.write().unwrap();

        if guard.is_disputed(&related_transaction_id) {
            return Err(anyhow!(TransactionError::AlreadyDisputedTransaction(
                related_transaction_id
            )));
        }
        if let Some(related_transaction) = guard.get_transaction(&related_transaction_id) {
            match related_transaction.kind {
                TransactionKind::Deposit(amount) => {
                    let mut account = guard.get_account(&related_transaction.client_id).unwrap(); // We know the account exists because the transaction exists.
                    account.dispute(amount)?;
                    guard.store_account(account)?;
                    guard.set_disputed(related_transaction_id, true)?;
                }
                _ => {
                    bail!(TransactionError::RelatedTransactionNotDisputable(
                        related_transaction_id
                    ));
                }
            }
        } else {
            bail!(TransactionError::RelatedTransactionNotFound(
                related_transaction_id
            ));
        }

        Ok(transaction)
    }

    /// Process a resolve order.
    fn process_resolve(
        &self,
        transaction: Transaction,
        related_transaction_id: TxId,
    ) -> Result<Transaction> {
        let mut guard = self.store.write().unwrap();

        if !guard.is_disputed(&related_transaction_id) {
            return Err(anyhow!(TransactionError::NonDisputedTransaction(
                related_transaction_id
            )));
        }
        let related_transaction = guard.get_transaction(&related_transaction_id).unwrap(); // We know the transaction exists because it is disputed.

        if let TransactionKind::Deposit(amount) = related_transaction.kind {
            let mut account = guard.get_account(&related_transaction.client_id).unwrap(); // We know the account exists because the transaction exists.
            account.resolve(amount)?;
            guard.store_account(account)?;
            guard.set_disputed(related_transaction_id, false)?;
        }

        Ok(transaction)
    }

    /// Process a chargeback order.
    fn process_chargeback(
        &self,
        transaction: Transaction,
        related_transaction_id: TxId,
    ) -> Result<Transaction> {
        let mut guard = self.store.write().unwrap();

        if !guard.is_disputed(&related_transaction_id) {
            return Err(anyhow!(TransactionError::NonDisputedTransaction(
                related_transaction_id
            )));
        }
        let related_transaction = guard.get_transaction(&related_transaction_id).unwrap(); // We know the transaction exists because it is disputed.

        if let TransactionKind::Deposit(amount) = related_transaction.kind {
            let mut account = guard.get_account(&related_transaction.client_id).unwrap(); // We know the account exists because the transaction exists.
            account.chargeback(amount)?;
            guard.store_account(account)?;
            guard.set_disputed(related_transaction_id, false)?;
        }

        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    use crate::adapter::InMemoryAccountStorage;

    use super::*;

    #[test]
    fn test_duplicate_disputable_transactions() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::ONE),
        };
        let _tx = manager.process_order(order.clone()).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::Withdrawal(Decimal::ONE),
        };
        let error = manager.process_order(order).unwrap_err();

        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::DuplicateTransactionId(tx_id)) if tx_id == &1
        ));
    }

    #[test]
    fn test_deposit() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let transaction = manager.process_order(order).unwrap();
        assert!(matches!(
            transaction.kind,
            TransactionKind::Deposit(amount) if amount == Decimal::TEN
        ));
        let account = manager.get_account(1).unwrap();
        assert_eq!(account.available, dec!(10));
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::ONE),
        };
        let _tx = manager.process_order(order).unwrap();
        let account = manager.get_account(1).unwrap();

        assert_eq!(account.available, dec!(11));
    }

    #[test]
    fn test_withdrawal() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 1,
            kind: TransactionKind::Withdrawal(Decimal::ONE),
        };
        let transaction = manager.process_order(order).unwrap();
        assert!(matches!(
            transaction.kind,
            TransactionKind::Withdrawal(amount) if amount == Decimal::ONE
        ));
        let account = manager.get_account(1).unwrap();
        assert_eq!(account.available, dec!(9));
    }

    #[test]
    fn test_dispute_ok() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Dispute(1),
        };
        let transaction = manager.process_order(order).unwrap();
        assert!(matches!(
            transaction.kind,
            TransactionKind::Dispute(related_tx_id) if related_tx_id == 1
        ));
        let account = manager.get_account(1).unwrap();
        assert_eq!(account.held, dec!(10));
        assert!(!account.locked);
    }

    #[test]
    fn test_dispute_non_existing_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 1,
            kind: TransactionKind::Dispute(2),
        };
        let error = manager.process_order(order).unwrap_err();

        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::RelatedTransactionNotFound(tx_id)) if tx_id == &2
        ));
    }

    #[test]
    fn test_dispute_a_non_deposit_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 1,
            kind: TransactionKind::Withdrawal(Decimal::ONE),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 2,
            kind: TransactionKind::Dispute(2),
        };
        let error = manager.process_order(order).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::RelatedTransactionNotDisputable(tx_id)) if tx_id == &2
        ));
    }

    #[test]
    fn dispute_an_already_disputed_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::Dispute(1),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 3,
            kind: TransactionKind::Dispute(1),
        };
        let error = manager.process_order(order).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::AlreadyDisputedTransaction(tx_id)) if tx_id == &1
        ));
    }

    #[test]
    fn resolve_a_disputed_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::Dispute(1),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::Resolve(1),
        };
        let transaction = manager.process_order(order).unwrap();
        assert!(matches!(
            transaction.kind,
            TransactionKind::Resolve(related_tx_id) if related_tx_id == 1
        ));
        let account = manager.get_account(1).unwrap();
        assert_eq!(account.available, dec!(10));
        assert_eq!(account.held, dec!(0));
    }

    #[test]
    fn resolve_a_non_disputed_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::Resolve(1),
        };
        let error = manager.process_order(order).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::NonDisputedTransaction(tx_id)) if tx_id == &1
        ));
    }

    #[test]
    fn resolve_a_non_existing_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 1,
            kind: TransactionKind::Resolve(2),
        };
        let error = manager.process_order(order).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::NonDisputedTransaction(tx_id)) if tx_id == &2
        ));
    }

    #[test]
    fn chargeback_a_disputed_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::Dispute(1),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::ChargeBack(1),
        };
        let transaction = manager.process_order(order).unwrap();
        assert!(matches!(
            transaction.kind,
            TransactionKind::ChargeBack(related_tx_id) if related_tx_id == 1
        ));
        let account = manager.get_account(1).unwrap();
        assert_eq!(account.available, dec!(0));
        assert_eq!(account.held, dec!(0));
        assert!(account.locked);
    }

    #[test]
    fn chargeback_a_non_disputed_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::TEN),
        };
        let _tx = manager.process_order(order).unwrap();
        let order = TransactionOrder {
            tx_id: 1,
            client_id: 2,
            kind: TransactionKind::ChargeBack(1),
        };
        let error = manager.process_order(order).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::NonDisputedTransaction(tx_id)) if tx_id == &1
        ));
        let account = manager.get_account(1).unwrap();
        assert_eq!(account.available, dec!(10));
        assert_eq!(account.held, dec!(0));
        assert!(!account.locked);
    }

    #[test]
    fn chargeback_a_non_existing_transaction() {
        let manager = AccountManager::new(InMemoryAccountStorage::default());
        let order = TransactionOrder {
            tx_id: 2,
            client_id: 1,
            kind: TransactionKind::ChargeBack(2),
        };
        let error = manager.process_order(order).unwrap_err();
        assert!(matches!(
            error.downcast_ref::<TransactionError>(),
            Some(TransactionError::NonDisputedTransaction(tx_id)) if tx_id == &2
        ));
    }
}
