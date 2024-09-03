use std::{
    io::{stdout, BufReader},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{anyhow, bail};
use clap::Parser;
use log::{debug, error, info};

use csv_reader::{
    actor::Accountant, adapter::InMemoryAccountStorage, model::TransactionOrder,
    service::AccountManager, Result,
};

/// Command line arguments
#[derive(Debug, Parser)]
struct CLIArguments {
    /// The path to the CSV file to read.
    csv_file: PathBuf,
}

struct Application {
    csv_file: PathBuf,
}

impl Application {
    fn new(csv_file: PathBuf) -> Result<Self> {
        if !csv_file.exists() {
            bail!("CSV file does not exist: '{:?}'.", csv_file.display());
        }
        if !csv_file.is_file() {
            bail!("CSV file is not a file: '{:?}'.", csv_file.canonicalize());
        }
        let this = Self { csv_file };

        Ok(this)
    }

    fn run(&self) -> Result<()> {
        info!("Starting CSV_READER version {}", env!("CARGO_PKG_VERSION"));
        debug!("Reading CSV file: '{:?}'.", self.csv_file.canonicalize());

        // dependencies
        // Create a channel to send orders to the accountant actor.
        let (order_sender, order_receiver) = std::sync::mpsc::channel::<TransactionOrder>();
        // Create a buffered reader for the CSV file.
        let buffer = BufReader::new(std::fs::File::open(&self.csv_file)?);

        // Create the accountant actor and start it in a separate thread.
        let account_manager = Arc::new(AccountManager::new(InMemoryAccountStorage::default()));
        let accountant_actor = Accountant::new(account_manager.clone(), order_receiver);
        let account_handler = std::thread::spawn(move || accountant_actor.run());

        // Create the reader actor and start it in a separate thread.
        let reader_actor = csv_reader::actor::Reader::new(order_sender, Box::new(buffer));
        let reader_handler = std::thread::spawn(move || reader_actor.run());

        reader_handler
            .join()
            .expect("Reader thread panicked")
            .and(account_handler.join().expect("Accountant thread panicked"))
            .map_err(|e| anyhow!("Threads returned an error: {:#?}", e))?; // Join the threads and propagate any error.

        // Export the accounts to a CSV file.
        csv_reader::actor::AccountExporter::new(account_manager, Box::new(stdout())).run()
    }
}
fn main() -> Result<()> {
    let arguments = CLIArguments::parse();
    let application = Application::new(arguments.csv_file)?;
    env_logger::init();

    let result = application.run();

    match &result {
        Ok(_) => {
            info!("CSV_READER completed successfully");
        }
        Err(error) => {
            error!("CSV_READER failed with error: {}", error);
        }
    };

    result
}
