use super::pager::Pager;
use super::table::{Table, TableBuilder};
use crate::error::Error;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub struct Database {
    pub pager: Rc<RefCell<Pager>>,
    pub tables: IndexMap<String, Table>,
}

impl Database {
    pub fn new(file_path: &str) -> Result<Self, Error> {
        Ok(Self {
            pager: Rc::new(RefCell::new(Pager::new(file_path)?)),
            tables: IndexMap::new(),
        })
    }

    /// In-memory database for testing (no file backing)
    pub fn memory() -> Self {
        Self {
            pager: Rc::new(RefCell::new(Pager::memory())),
            tables: IndexMap::new(),
        }
    }

    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name)
    }

    pub fn table_exists(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    pub fn create_table(&mut self, builder: TableBuilder) -> Result<(), Error> {
        let table = builder.build(Rc::clone(&self.pager))?;
        if self.table_exists(&table.name) {
            return Err(Error::DuplicateTable(table.name.clone()));
        }
        self.tables.insert(table.name.clone(), table);
        Ok(())
    }
}
