use crate::error::Error;
use crate::sql::ast::*;
use crate::storage::table::*;
use crate::types::*;

enum LogicalPlan {
    Scan {
        table: String,
    },
    Filter {
        input: Box<LogicalPlan>,
        predicate: Expression,
    },
    Project {
        input: Box<LogicalPlan>,
        columns: Vec<String>,
    },
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        on: Expression,
    },
}
