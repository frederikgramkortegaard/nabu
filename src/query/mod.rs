mod analyzer;
mod resolved;

pub use analyzer::resolve;
pub use resolved::*;
pub use crate::shared::Value;

#[derive(Debug)]
pub enum QueryResult {
    Insert { rows_affected: u64 },
    Select { rows: Vec<Vec<Value>> },
    Delete { rows_affected: u64 },
}
