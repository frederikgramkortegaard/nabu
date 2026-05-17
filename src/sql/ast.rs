use ordered_float::OrderedFloat;
use std::cmp::Ordering;
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Varchar(usize),
    Bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Number(OrderedFloat<f64>),
    Varchar(String),
    Bool(bool),
}
impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Varchar(a), Value::Varchar(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            _ => panic!(
                "Cannot compare different       
  types"
            ),
        }
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Value {
    pub fn serialize(&self, dest: &mut [u8], column_size: usize) {
        match self {
            Value::Number(value) => {
                dest[..column_size].copy_from_slice(&value.to_le_bytes());
            }

            // @TODO : Currently we just null-pad these
            Value::Varchar(value) => {
                let bytes = value.as_bytes();
                let len = bytes.len().min(column_size);
                dest[..len].copy_from_slice(&bytes[..len]);
                dest[len..column_size].fill(0);
            }

            Value::Bool(value) => {
                dest[0] = if *value { 1 } else { 0 };
            }
        }
    }
    pub fn typ(&self) -> Type {
        match self {
            Value::Number(_) => Type::Number,
            Value::Varchar(s) => Type::Varchar(s.len()),
            Value::Bool(_) => Type::Bool,
        }
    }

    pub fn leq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a <= b,
            (Value::Varchar(a), Value::Varchar(b)) => a <= b,
            (Value::Bool(a), Value::Bool(b)) => *a as u8 <= *b as u8,
            _ => panic!("Cannot compare values of different types"),
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
