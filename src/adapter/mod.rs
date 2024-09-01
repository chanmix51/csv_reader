//! The adapter module holds the implementation of tools required by the services.
//! Thes different adapters perform operation that involve IOs like reading or
//! writing to files or databases. (more geneally, the outside world)

mod account_storage;

pub use account_storage::*;
