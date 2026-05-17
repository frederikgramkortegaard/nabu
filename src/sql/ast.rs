pub use crate::value::{Type, Value};

#[derive(Debug)]
pub struct InsertStatement {
    pub values: Vec<Value>,
    pub table_name: String,
}

#[derive(Debug)]
pub enum Operator {
    Eq,
    Neq,
    Leq,
    Geq,
    Lt,
    Gt,

    Add,
    Sub,
    Mul,
    Div,

    And,
    Or,
}

#[derive(Debug)]
pub enum Expression {
    Literal(Value),
    Identifier(String),
    BinaryOp {
        op: Operator,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },
}

#[derive(Debug)]
pub struct SelectStatement {
    pub columns: Vec<String>,
    pub table: String,
    pub expr: Option<Box<Expression>>,
}

#[derive(Debug)]
pub struct DeleteStatement {
    pub table: String,
    pub expr: Option<Box<Expression>>,
}

#[derive(Debug)]
pub enum Statement {
    Insert(InsertStatement),
    Select(SelectStatement),
    Delete(DeleteStatement),
}
