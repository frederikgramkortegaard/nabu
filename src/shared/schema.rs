use arrow::datatypes::{FieldRef, Schema, SchemaRef};
use std::collections::HashMap;
use std::sync::Arc;

use super::field::FieldExt;

/// Extension trait for Schema - adds our database-specific functionality
pub trait SchemaExt {
    fn table_name(&self) -> Option<&str>;
    fn row_size(&self) -> usize;
}

impl SchemaExt for Schema {
    fn table_name(&self) -> Option<&str> {
        self.metadata().get("table_name").map(|s| s.as_str())
    }

    fn row_size(&self) -> usize {
        self.fields().iter().map(|f| f.byte_size()).sum()
    }
}

/// Helper to create a Schema with a table name
pub fn new_table_schema(table_name: &str, fields: Vec<FieldRef>) -> SchemaRef {
    let metadata = HashMap::from([("table_name".to_string(), table_name.to_string())]);
    Arc::new(Schema::new_with_metadata(fields, metadata))
}

/// Helper to create a Field with a storage size (returns Arc<Field>)
pub fn new_field(
    name: &str,
    data_type: arrow::datatypes::DataType,
    nullable: bool,
    size: usize,
) -> FieldRef {
    let metadata = HashMap::from([("size".to_string(), size.to_string())]);
    Arc::new(arrow::datatypes::Field::new(name, data_type, nullable).with_metadata(metadata))
}
