//! # Actor module
//!
//! The actors are controlers. They use services to perform their tasks.
//! They communicate with other actors through messages.

mod accountant;

pub use accountant::*;
