use crate::catalog::Catalog;
use arrow::array::*;

pub trait Provider: Catalog {
    // TODO: data access methods for execution
    //
    fn scan(projection: &[&str]) -> RecordBatch {
        todo!("Provider::scan not implemented")
    }
}
