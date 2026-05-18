use super::value::{Type, Value};

pub type Row = Vec<Value>;

#[derive(Debug, Clone, Copy)]
pub enum ColumnType {
    Number,
    Varchar(usize),
    Bool,
}

impl ColumnType {
    pub fn size(self) -> usize {
        match self {
            ColumnType::Number => 8,
            ColumnType::Varchar(n) => n,
            ColumnType::Bool => 1,
        }
    }

    pub fn to_type(self) -> Type {
        match self {
            ColumnType::Number => Type::Number,
            ColumnType::Varchar(n) => Type::Varchar(n),
            ColumnType::Bool => Type::Bool,
        }
    }
    pub fn to_string(&self) -> String {
        match self {
            ColumnType::Number => "n".to_string(),
            ColumnType::Bool => "b".to_string(),
            ColumnType::Varchar(n) => format!("v{}", n),
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "n" => Some(ColumnType::Number),
            "b" => Some(ColumnType::Bool),
            s if s.starts_with('v') => s[1..].parse().ok().map(ColumnType::Varchar),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub column_type: ColumnType,
    pub column_size: usize,
}

impl Column {
    pub fn to_string(&self) -> String {
        format!("{}:{}", self.name, self.column_type.to_string())
    }
    pub fn new(name: String, column_type: ColumnType) -> Self {
        Column {
            name,
            column_size: column_type.size(),
            column_type,
        }
    }
}

pub fn serialize_row(values: &[Value], columns: Vec<&Column>, dest: &mut [u8]) {
    let mut offset = 0;
    for (value, col) in values.iter().zip(columns.iter()) {
        value.serialize(&mut dest[offset..], col.column_size);
        offset += col.column_size;
    }
}

pub fn deserialize_row(columns: &Vec<&Column>, src: &[u8]) -> Row {
    let mut values = Row::with_capacity(columns.len());
    let mut offset = 0;
    for column in columns {
        values.push(Value::deserialize(&src[offset..], column));
        offset += column.column_size;
    }
    values
}
