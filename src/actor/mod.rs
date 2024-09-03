//! # Actor module
//!
//! The actors are controlers. They use services to perform their tasks.
//! They communicate with other actors through messages.

mod accountant;
mod reader;

pub use accountant::*;
pub use reader::*;
