//! CVS READER LIBRARY
//!
//! This library provides elements to read transaction data from a CSV file and
//! compute accounts from it.

pub mod model;

pub type Result<T> = anyhow::Result<T>;
