//! Reader actor
//!
//! The reader actor is responsible for reading the transaction data from a CSV
//! file.  The actor reads the file line by line and send the transaction orders
//! to the accountant actor through a channel.

use std::{io::Read, sync::mpsc::Sender};

use csv::ReaderBuilder;
use log::debug;

use crate::model::{CSVTransactionEntity, TransactionOrder};

/// Reader actor.
pub struct Reader {
    /// The order channel sender to send transaction orders.
    order_sender: Sender<TransactionOrder>,
    reader: Box<dyn Read + Sync + Send>,
}

impl Reader {
    /// Create a new reader actor.
    pub fn new(
        order_sender: Sender<TransactionOrder>,
        reader: Box<dyn Read + Sync + Send>,
    ) -> Self {
        Self {
            order_sender,
            reader,
        }
    }

    /// Run the reader actor.
    /// The actor will read the CSV file line by line and send the transaction
    /// orders to the accountant actor through the order channel.
    pub fn run(self) -> crate::Result<()> {
        debug!("Reader Actor started");
        let mut csv_reader = ReaderBuilder::new()
            .has_headers(true)
            .trim(csv::Trim::All)
            .from_reader(Box::leak(self.reader));

        for result in csv_reader.deserialize() {
            let record: CSVTransactionEntity = match result {
                Err(error) => {
                    log::info!("Error reading CSV record: {}", error);
                    continue;
                }
                Ok(record) => record,
            };
            let order = match TransactionOrder::try_from(record) {
                Err(error) => {
                    log::info!("Error parsing CSV record: {}", error);
                    continue;
                }
                Ok(order) => order,
            };

            self.order_sender.send(order)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::channel;

    fn assert_run_ok(data: &'static str, ok_lines: usize) {
        let (tx, rx) = channel();
        let actor = Reader::new(tx, Box::new(data.as_bytes()));
        let handler = std::thread::spawn(move || actor.run());

        assert!(handler.join().unwrap().is_ok());
        let orders: Vec<TransactionOrder> = rx.iter().collect();
        assert_eq!(orders.len(), ok_lines);
    }

    #[test]
    fn simple_ok_sample() {
        let data = r#"type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2
withdrawal, 1, 4, 1.500
withdrawal, 2, 5, 3.0"#;
        assert_run_ok(data, 5);
    }

    #[test]
    fn test_mixed_case() {
        let data = r#"type, client, tx, amount
Deposit,1,1,1.0
dEpOsiT, 2, 2, 2.0  
DEPOSIT, 1, 3, 2
Withdrawal, 1, 4, 1.500"#;
        assert_run_ok(data, 4);
    }

    #[test]
    fn test_empty_lines() {
        let data = r#"type, client, tx, amount
deposit, 1, 1 , 1.0
deposit, 2, 2, 2.0  

deposit, 1, 3, 2
withdrawal, 1, 4, 1.500

withdrawal, 2, 5, 3.0

"#;
        assert_run_ok(data, 5);
    }

    #[test]
    fn test_extra_spaces() {
        let data = r#"type, client, tx, amount
   deposit, 1, 1 , 1.0
deposit, 2, 2, 2.0    
deposit    , 1, 3, 2
withdrawal, 1,   4, 1.500
withdrawal, 2, 5, 3.0     "#;
        assert_run_ok(data, 5);
    }

    #[test]
    fn test_invalid_transaction_kind() {
        let data = r#"type, client, tx, amount
deposit, 1, 1, 1.0
whatever, 1, 2, 2.0
withdrawal, 1,   4, 1.500
dispute, 2, 5, 1"#;
        assert_run_ok(data, 3);
    }
}
