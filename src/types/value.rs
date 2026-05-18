use super::column::{Column, ColumnType};
use ordered_float::OrderedFloat;
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Number,
    Varchar(usize),
    Bool,
    Bytes(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Number(OrderedFloat<f64>),
    Varchar(String),
    Bool(bool),
    Bytes(Vec<u8>),
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Varchar(a), Value::Varchar(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Bytes(a), Value::Bytes(b)) => a.cmp(b),
            _ => panic!("Cannot compare different types"),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Value {
    pub fn typ(&self) -> Type {
        match self {
            Value::Number(_) => Type::Number,
            Value::Varchar(s) => Type::Varchar(s.len()),
            Value::Bool(_) => Type::Bool,
            Value::Bytes(b) => Type::Bytes(b.len()),
        }
    }

    pub fn leq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => a <= b,
            (Value::Varchar(a), Value::Varchar(b)) => a <= b,
            (Value::Bool(a), Value::Bool(b)) => *a as u8 <= *b as u8,
            (Value::Bytes(a), Value::Bytes(b)) => a <= b,
            _ => panic!("Cannot compare values of different types"),
        }
    }

    pub fn serialize(&self, dest: &mut [u8], column_size: usize) {
        match self {
            Value::Number(value) => {
                dest[..column_size].copy_from_slice(&value.to_le_bytes());
            }
            Value::Varchar(value) => {
                let bytes = value.as_bytes();
                let len = bytes.len().min(column_size);
                dest[..len].copy_from_slice(&bytes[..len]);
                dest[len..column_size].fill(0);
            }
            Value::Bool(value) => {
                dest[0] = if *value { 1 } else { 0 };
            }
            Value::Bytes(value) => {
                let len = value.len().min(column_size);
                dest[..len].copy_from_slice(&value[..len]);
                dest[len..column_size].fill(0);
            }
        }
    }

    pub fn deserialize(src: &[u8], col: &Column) -> Value {
        match col.column_type {
            ColumnType::Number => {
                let bytes: [u8; 8] = src[..col.column_size].try_into().unwrap();
                Value::Number(OrderedFloat(f64::from_le_bytes(bytes)))
            }
            ColumnType::Varchar(max_len) => {
                let bytes = &src[..max_len];
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(max_len);
                Value::Varchar(String::from_utf8_lossy(&bytes[..end]).into_owned())
            }
            ColumnType::Bool => Value::Bool(src[0] != 0),
            ColumnType::Bytes(len) => Value::Bytes(src[..len].to_vec()),
        }
    }
}
