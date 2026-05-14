use super::pager::Pager;
use crate::sql::ast::Value;
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy)]
pub enum ColumnType {
    Number,
    Varchar(usize),
}

impl ColumnType {
    pub fn size(self) -> usize {
        match self {
            ColumnType::Number => 8,     // 8 Bytes == 64 Bits
            ColumnType::Varchar(n) => n, // 1 Byte == 8 Bits per Character,
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
    pub fn new(name: String, column_type: ColumnType) -> Self {
        Column {
            name,
            column_type,
            column_size: column_type.size(),
        }
    }
}

pub fn serialize_row(values: &[Value], columns: Vec<&Column>, dest: &mut [u8]) {
    let mut offset = 0;
    for (value, column) in std::iter::zip(values, columns) {
        match value {
            Value::Number(value) => {
                dest[offset..offset + column.column_type.size()]
                    .copy_from_slice(&value.to_le_bytes());
                offset += column.column_type.size();
            }

            // @TODO : Currently we just null-padd these
            Value::Varchar(value) => {
                let bytes = value.as_bytes();
                let len = bytes.len().min(column.column_size);
                dest[offset..offset + len].copy_from_slice(&bytes[..len]);
                dest[offset + len..offset + column.column_size].fill(0);
                offset += column.column_size;
            }
        }
    }
}

pub fn deserialize_row(columns: Vec<&Column>, src: &[u8]) {
    let mut values: Vec<Value> = Vec::with_capacity(columns.len());
    let mut offset = 0;
    for column in columns {
        match column.column_type {
            ColumnType::Number => {
                let bytes = src[offset..offset + column.column_size].try_into().unwrap();
                let num = f64::from_le_bytes(bytes);
                offset += column.column_size;
                values.push(Value::Number(num))
            }

            ColumnType::Varchar(max_len) => {
                let bytes = &src[offset..offset + max_len];
                let end = bytes.iter().position(|&b| b == 0).unwrap_or(max_len);
                let s = String::from_utf8_lossy(&bytes[..end]).into_owned();
                offset += column.column_size;
                values.push(Value::Varchar(s));
            }
        };
    }
}
#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub columns: IndexMap<String, Column>,
    pub row_size: usize,
    pub pager: Pager,
}

impl Table {
    pub fn new(name: String, columns: impl IntoIterator<Item = (String, ColumnType)>) -> Self {
        let columns: IndexMap<String, Column> = columns
            .into_iter()
            .map(|(name, column_type)| (name.clone(), Column::new(name, column_type)))
            .collect();

        let row_size: u64 = columns.iter().map(|(_, c)| c.column_size as u64).sum();

        Table {
            name,
            columns,
            row_size: row_size as usize,
            pager: Pager::new(),
        }
    }
}
