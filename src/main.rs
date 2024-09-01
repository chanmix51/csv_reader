use log::info;

fn main() {
    env_logger::init();
    info!("Starting CSV_READER version {}", env!("CARGO_PKG_VERSION"));
}
