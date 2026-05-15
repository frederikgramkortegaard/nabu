use crate::sql::ast::Value;

#[derive(Debug)]
pub enum QueryResult {
    Insert { rows_affected: u64 },
    Select { rows: Vec<Vec<Value>> },
}
