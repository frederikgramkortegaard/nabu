use crate::provider::TableProvider;
use crate::shared::SchemaRef;
use std::sync::Arc;

pub trait Catalog {
    fn get_schema(&self, name: &str) -> Option<SchemaRef>;
    fn get_schemas(&self) -> Vec<SchemaRef>;
    fn table(&self, name: &str) -> Option<Arc<dyn TableProvider>>;
}
