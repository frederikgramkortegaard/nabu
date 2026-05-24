use crate::error::Error;
use crate::shared::SchemaRef;
use arrow::record_batch::RecordBatch;

pub trait TableProvider {
    fn schema(&self) -> SchemaRef;
    fn scan(&self, projection: Option<&[usize]>) -> Result<Box<dyn Iterator<Item = Result<RecordBatch, Error>> + '_>, Error>;
}
