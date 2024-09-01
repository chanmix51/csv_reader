//! Service module
//!
//! Services can be considered as the business logic of the application. They are like
//! applications used by the actors to perform operations on the data. They are
//! responsible for managing the data and the operations that can be performed
//! on it. They must ensure that the data is consistent and that the operations
//! are performed correctly.

mod account_manager;

pub use account_manager::*;
