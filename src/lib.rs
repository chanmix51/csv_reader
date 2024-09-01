#![warn(missing_docs)]
//! CVS READER LIBRARY
//!
//! This library provides elements to read transaction data from a CSV file and
//! compute accounts from it.

pub mod adapter;
pub mod model;

/// Global type alias for the result type used in this library.
pub type Result<T> = anyhow::Result<T>;
