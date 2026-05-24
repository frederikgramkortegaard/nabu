use crate::frontend::ast::{JoinKind, Operator};
use crate::shared::{FieldRef, SchemaRef};
use super::Value;

#[derive(Debug, Clone)]
pub enum ResolvedExpression {
    Literal(Value),
    Column(FieldRef),  // Arc<Field> - cheap to clone
    BinaryOp {
        op: Operator,
        lhs: Box<ResolvedExpression>,
        rhs: Box<ResolvedExpression>,
    },
}

#[derive(Debug)]
pub enum ResolvedStatement {
    Insert {
        schema: SchemaRef,
        values: Vec<Value>,
    },
    Select {
        schema: SchemaRef,
        columns: Vec<FieldRef>,
        joins: Vec<ResolvedJoin>,
        filter: Option<ResolvedExpression>,
        limit: Option<usize>,
        offset: Option<usize>,
        order_by: Option<FieldRef>,
    },
    Delete {
        schema: SchemaRef,
        filter: Option<ResolvedExpression>,
    },
}

#[derive(Debug)]
pub struct ResolvedJoin {
    pub kind: JoinKind,
    pub schema: SchemaRef,
    pub left_col: FieldRef,
    pub right_col: FieldRef,
}
