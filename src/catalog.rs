use crate::shared::SchemaRef;

pub trait Catalog {
    fn get_schema(&self, name: &str) -> Option<SchemaRef>;
    fn get_schemas(&self) -> Vec<SchemaRef>;
}
