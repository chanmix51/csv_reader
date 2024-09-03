//! The accountant actor is responsible for managing the transactions and accounts of the clients.
//! For that purpose, it uses the [AccountManager] service.

use std::sync::{mpsc::Receiver, Arc};

use crate::{model::TransactionOrder, service::AccountManager};

/// The accountant actor is responsible for managing the transactions and
/// accounts of the clients.
pub struct Accountant {
    /// The account manager service.
    account_manager: Arc<AccountManager>,

    /// The order channel receiver to read transaction orders.
    order_receiver: Receiver<TransactionOrder>,
}

impl Accountant {
    /// Create a new accountant actor.
    pub fn new(
        account_manager: Arc<AccountManager>,
        order_receiver: Receiver<TransactionOrder>,
    ) -> Self {
        Self {
            account_manager,
            order_receiver,
        }
    }

    /// Run the accountant actor.
    /// The actor will process the orders received from the order channel.
    /// It will NOT stop when the transactions fail but only log the error if any.
    /// The actor will stop when the order channel is closed which means that no
    /// more orders will be received.
    pub fn run(&self) {
        for order in self.order_receiver.iter() {
            if let Err(error) = self.account_manager.process_order(order) {
                log::info!("Error processing order: {}", error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    use std::sync::mpsc::channel;

    use crate::{adapter::InMemoryAccountStorage, model::TransactionKind, service::AccountManager};

    #[test]
    fn test_run() {
        let (tx, rx) = channel();
        let account_manager = Arc::new(AccountManager::new(InMemoryAccountStorage::default()));
        let accountant = Accountant::new(account_manager.clone(), rx);
        let handler = std::thread::spawn(move || {
            accountant.run();
        });
        tx.send(TransactionOrder {
            tx_id: 1,
            client_id: 1,
            kind: TransactionKind::Deposit(Decimal::ONE_HUNDRED),
        })
        .unwrap();
        // Dispute a non-existing transaction
        // This should not fail but log an error
        tx.send(TransactionOrder {
            tx_id: 2,
            client_id: 2,
            kind: TransactionKind::Dispute(3),
        })
        .unwrap();
        tx.send(TransactionOrder {
            tx_id: 3,
            client_id: 1,
            kind: TransactionKind::Withdrawal(Decimal::ONE),
        })
        .unwrap();
        // Send twice the same transaction
        // It must not be taken into account
        tx.send(TransactionOrder {
            tx_id: 3,
            client_id: 1,
            kind: TransactionKind::Withdrawal(Decimal::ONE),
        })
        .unwrap();
        drop(tx);
        handler.join().unwrap();
        let account = account_manager.get_account(1).unwrap();

        assert_eq!(account.available, Decimal::ONE_HUNDRED - Decimal::ONE);
    }
}
