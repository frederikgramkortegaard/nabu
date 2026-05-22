mod column;
mod transaction;
mod value;

pub use column::{Column, ColumnType, Row};
pub use value::{Type, Value};

#[derive(Debug)]
pub enum QueryResult {
    Insert { rows_affected: u64 },
    Select { rows: Vec<Vec<Value>> },
    Delete { rows_affected: u64 },
}
