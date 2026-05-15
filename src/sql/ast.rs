#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Varchar(usize),
    Bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Varchar(String),
    Bool(bool),
}

impl Value {
    pub fn typ(&self) -> Type {
        match self {
            Value::Number(_) => Type::Number,
            Value::Varchar(s) => Type::Varchar(s.len()),
            Value::Bool(_) => Type::Bool,
        }
    }
}

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
pub enum Statement {
    Insert(InsertStatement),
    Select(SelectStatement),
}
