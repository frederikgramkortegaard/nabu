use super::ResolvedExpression;
use crate::frontend::ast::JoinKind;
use crate::shared::{FieldRef, SchemaRef, Value};

#[derive(Debug)]
pub enum LogicalPlan {
    Scan {
        table_name: String,
        schema: SchemaRef,
        projection: Option<Vec<usize>>,
    },
    Insert {
        table_name: String,
        schema: SchemaRef,
        values: Vec<Value>,
    },
    Filter {
        predicate: ResolvedExpression,
        input: Box<LogicalPlan>,
    },
    Projection {
        expr: Vec<ResolvedExpression>,
        input: Box<LogicalPlan>,
    },
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        on: Vec<(FieldRef, FieldRef)>,
        join_type: JoinKind,
    },
    Sort {
        by: Vec<(ResolvedExpression, bool)>,
        input: Box<LogicalPlan>,
    },
    Limit {
        limit: Option<usize>,
        offset: Option<usize>,
        input: Box<LogicalPlan>,
    },
    /* @TODO
     * Aggregate (frontend needs to support group_by clause)
     * Distinct
     */
}
