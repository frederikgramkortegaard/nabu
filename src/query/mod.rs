mod analyzer;
mod logical;
mod plans;
mod resolved;

pub use crate::shared::Value;
pub use analyzer::resolve;
pub use logical::structure;
pub use plans::*;
pub use resolved::*;

#[derive(Debug)]
pub enum QueryResult {
    Insert { rows_affected: u64 },
    Select { rows: Vec<Vec<Value>> },
    Delete { rows_affected: u64 },
}
