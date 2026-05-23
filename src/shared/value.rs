use arrow::datatypes::{DataType, Field};
use ordered_float::OrderedFloat;
use std::cmp::Ordering;

/// Scalar value - represents a single value of any supported type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Float64(OrderedFloat<f64>),
    Utf8(String),
    Boolean(bool),
    Binary(Vec<u8>),
    Null,
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::Float64(a), Value::Float64(b)) => a.cmp(b),
            (Value::Utf8(a), Value::Utf8(b)) => a.cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),
            (Value::Binary(a), Value::Binary(b)) => a.cmp(b),
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
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
    pub fn data_type(&self) -> DataType {
        match self {
            Value::Float64(_) => DataType::Float64,
            Value::Utf8(_) => DataType::Utf8,
            Value::Boolean(_) => DataType::Boolean,
            Value::Binary(_) => DataType::Binary,
            Value::Null => DataType::Null,
        }
    }

    pub fn leq(&self, other: &Value) -> bool {
        self <= other
    }

    /// Serialize value to bytes with fixed size
    pub fn serialize(&self, dest: &mut [u8], size: usize) {
        match self {
            Value::Float64(value) => {
                dest[..8].copy_from_slice(&value.to_le_bytes());
            }
            Value::Utf8(value) => {
                let bytes = value.as_bytes();
                let len = bytes.len().min(size);
                dest[..len].copy_from_slice(&bytes[..len]);
                dest[len..size].fill(0);
            }
            Value::Boolean(value) => {
                dest[0] = if *value { 1 } else { 0 };
            }
            Value::Binary(value) => {
                let len = value.len().min(size);
                dest[..len].copy_from_slice(&value[..len]);
                dest[len..size].fill(0);
            }
            Value::Null => {
                dest[..size].fill(0);
            }
        }
    }

    /// Deserialize value from bytes based on field definition
    pub fn deserialize(src: &[u8], field: &Field) -> Value {
        match field.data_type() {
            DataType::Float64 => {
                let bytes: [u8; 8] = src[..8].try_into().unwrap();
                Value::Float64(OrderedFloat(f64::from_le_bytes(bytes)))
            }
            DataType::Utf8 => {
                // For now, read until null terminator or end of buffer
                let end = src.iter().position(|&b| b == 0).unwrap_or(src.len());
                Value::Utf8(String::from_utf8_lossy(&src[..end]).into_owned())
            }
            DataType::Boolean => Value::Boolean(src[0] != 0),
            DataType::Binary => Value::Binary(src.to_vec()),
            DataType::FixedSizeBinary(len) => {
                // Fixed-size binary used for fixed-size strings (CHAR(n))
                let data = &src[..*len as usize];
                let end = data.iter().position(|&b| b == 0).unwrap_or(*len as usize);
                Value::Utf8(String::from_utf8_lossy(&data[..end]).into_owned())
            }
            DataType::Null => Value::Null,
            dt => panic!("Unsupported data type for deserialization: {:?}", dt),
        }
    }
}
