use super::value::{Type, Value};
use crate::constants::{BOOL_SIZE, MAX_VARCHAR_LEN, NUMBER_SIZE};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct Row(pub Vec<Value>);
impl Row {
    pub fn new() -> Self {
        Row(Vec::new())
    }

    pub fn with_capacity(size: usize) -> Self {
        Row(Vec::with_capacity(size))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn deserialize(columns: &Vec<&Column>, src: &[u8]) -> Row {
        let mut values = Row::with_capacity(columns.len());
        let mut offset = 0;
        for column in columns {
            values.push(Value::deserialize(&src[offset..], column));
            offset += column.column_size;
        }
        values
    }
    pub fn serialize(&self, columns: Vec<&Column>, dest: &mut [u8]) {
        let mut offset = 0;
        for (value, col) in self.0.iter().zip(columns.iter()) {
            value.serialize(&mut dest[offset..], col.column_size);
            offset += col.column_size;
        }
    }
}

impl DerefMut for Row {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Deref for Row {
    type Target = Vec<Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[derive(Debug, Clone, Copy)]
pub enum ColumnType {
    Number,
    Varchar(usize),
    Bool,
    Bytes(usize),
}

impl ColumnType {
    pub fn size(self) -> usize {
        match self {
            ColumnType::Number => NUMBER_SIZE,
            ColumnType::Varchar(n) => n,
            ColumnType::Bool => BOOL_SIZE,
            ColumnType::Bytes(n) => n,
        }
    }

    pub fn to_type(self) -> Type {
        match self {
            ColumnType::Number => Type::Number,
            ColumnType::Varchar(n) => Type::Varchar(n),
            ColumnType::Bool => Type::Bool,
            ColumnType::Bytes(n) => Type::Bytes(n),
        }
    }
}

impl std::fmt::Display for ColumnType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnType::Number => write!(f, "n"),
            ColumnType::Bool => write!(f, "b"),
            ColumnType::Varchar(n) => write!(f, "v{}", n),
            ColumnType::Bytes(n) => write!(f, "y{}", n),
        }
    }
}

impl std::str::FromStr for ColumnType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "n" => Ok(ColumnType::Number),
            "b" => Ok(ColumnType::Bool),
            s if s.starts_with('v') => s[1..]
                .parse()
                .map(ColumnType::Varchar)
                .map_err(|_| format!("invalid varchar size: {}", &s[1..])),
            s if s.starts_with('y') => s[1..]
                .parse()
                .map(ColumnType::Bytes)
                .map_err(|_| format!("invalid bytes size: {}", &s[1..])),
            _ => Err(format!("unknown column type: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub column_type: ColumnType,
    pub column_size: usize,
}

impl std::fmt::Display for Column {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.name, self.column_type)
    }
}

impl std::str::FromStr for Column {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, type_str) = s
            .split_once(':')
            .ok_or_else(|| format!("invalid column format: {}", s))?;
        let column_type: ColumnType = type_str.parse()?;
        Ok(Column::new(name.to_string(), column_type))
    }
}

impl Column {
    pub fn new(name: String, column_type: ColumnType) -> Self {
        Column {
            name,
            column_size: column_type.size(),
            column_type,
        }
    }
}
