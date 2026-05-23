use crate::shared::Value;
use crate::shared::{Field, FieldExt};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
pub struct Record(pub Vec<Value>);

impl Record {
    pub fn new() -> Self {
        Record(Vec::new())
    }

    pub fn with_capacity(size: usize) -> Self {
        Record(Vec::with_capacity(size))
    }

    pub fn deserialize(columns: &[&Field], src: &[u8]) -> Record {
        let mut values = Record::with_capacity(columns.len());
        let mut offset = 0;
        for column in columns {
            let size = column.byte_size();
            values.push(Value::deserialize(&src[offset..], column));
            offset += size;
        }
        values
    }

    pub fn serialize(&self, columns: &[&Field], dest: &mut [u8]) {
        let mut offset = 0;
        for (value, col) in self.0.iter().zip(columns.iter()) {
            let size = col.byte_size();
            value.serialize(&mut dest[offset..], size);
            offset += size;
        }
    }
}

impl DerefMut for Record {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for Record {
    type Target = Vec<Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
