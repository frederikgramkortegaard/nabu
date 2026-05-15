use super::pager::{PAGE_SIZE, Pager};
use crate::sql::ast::{Type, Value};
use crate::types::Cursor;
use indexmap::IndexMap;
use std::cell::{Cell, RefCell};

#[derive(Debug)]
pub enum TableError {
    ReservedColumnName(String),
    DuplicateColumn(String),
    NoColumns,
}

#[derive(Debug, Clone, Copy)]
pub enum ColumnType {
    Number,
    Varchar(usize),
    Bool,
}

impl ColumnType {
    pub fn size(self) -> usize {
        match self {
            ColumnType::Number => 8,     // 8 Bytes == 64 Bits
            ColumnType::Varchar(n) => n, // 1 Byte == 8 Bits per Character
            ColumnType::Bool => 1,       // 1 Byte
        }
    }

    pub fn to_type(self) -> Type {
        match self {
            ColumnType::Number => Type::Number,
            ColumnType::Varchar(n) => Type::Varchar(n),
            ColumnType::Bool => Type::Bool,
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
                dest[offset..offset + column.column_size].copy_from_slice(&value.to_le_bytes());
                offset += column.column_size;
            }

            // @TODO : Currently we just null-pad these
            Value::Varchar(value) => {
                let bytes = value.as_bytes();
                let len = bytes.len().min(column.column_size);
                dest[offset..offset + len].copy_from_slice(&bytes[..len]);
                dest[offset + len..offset + column.column_size].fill(0);
                offset += column.column_size;
            }

            Value::Bool(value) => {
                dest[offset] = if *value { 1 } else { 0 };
                offset += 1;
            }
        }
    }
}

pub fn deserialize_row(columns: &Vec<&Column>, src: &[u8]) -> Vec<Value> {
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

            ColumnType::Bool => {
                let value = src[offset] != 0;
                offset += 1;
                values.push(Value::Bool(value));
            }
        };
    }
    values
}

#[derive(Debug)]
pub struct TableBuilder {
    name: String,
    columns: IndexMap<String, Column>,
    error: Option<TableError>,
}

impl TableBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        TableBuilder {
            name: name.into(),
            columns: IndexMap::new(),
            error: None,
        }
    }

    pub fn column(mut self, name: impl Into<String>, column_type: ColumnType) -> Self {
        if self.error.is_some() {
            return self;
        }
        let name = name.into();
        if name.starts_with('_') {
            self.error = Some(TableError::ReservedColumnName(name));
            return self;
        }
        if self.columns.contains_key(&name) {
            self.error = Some(TableError::DuplicateColumn(name));
            return self;
        }
        self.columns
            .insert(name.clone(), Column::new(name, column_type));
        self
    }

    pub fn build(self) -> Result<Table, TableError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if self.columns.is_empty() {
            return Err(TableError::NoColumns);
        }
        Ok(Table::from_columns(self.name, self.columns))
    }
}

#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub columns: IndexMap<String, Column>,
    pub rows: Cell<usize>,
    pub row_size: usize,
    pub rows_per_page: usize,
    pub pager: RefCell<Pager>,
}

impl Table {
    pub fn new(name: String, columns: impl IntoIterator<Item = (String, ColumnType)>) -> Self {
        let columns: IndexMap<String, Column> = columns
            .into_iter()
            .map(|(name, column_type)| (name.clone(), Column::new(name, column_type)))
            .collect();

        Self::from_columns(name, columns)
    }

    pub fn get_column(&self, column_name: &str) -> Option<&Column> {
        self.columns.get(column_name)
    }

    pub fn start<'a>(&'a self) -> Cursor<'a> {
        Cursor {
            table: self,
            row: 0,
            eot: self.rows.get() == 0,
        }
    }
    pub fn end<'a>(&'a self) -> Cursor<'a> {
        Cursor {
            table: self,
            row: self.rows.get(),
            eot: true,
        }
    }

    fn from_columns(name: String, user_columns: IndexMap<String, Column>) -> Self {
        // Prepend built-in columns
        let mut columns = IndexMap::new();
        columns.insert(
            "_rowid".to_string(),
            Column::new("_rowid".to_string(), ColumnType::Number),
        );
        columns.extend(user_columns);

        let row_size: usize = columns.iter().map(|(_, c)| c.column_size).sum();
        let rows_per_page = PAGE_SIZE / row_size;

        Table {
            name,
            columns,
            rows: Cell::new(0),
            row_size,
            rows_per_page,
            pager: RefCell::new(Pager::new()),
        }
    }

    /// Returns only user-defined columns (excludes system columns like _rowid)
    pub fn user_columns(&self) -> impl Iterator<Item = &Column> {
        self.columns.values().filter(|c| !c.name.starts_with('_'))
    }
}
