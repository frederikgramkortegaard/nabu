use super::pager::Pager;
use super::table::{Table, TableBuilder};
use crate::error::Error;
use crate::magic::DatabaseHeader;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub struct Database {
    pub pager: Rc<RefCell<Pager>>,
    pub tables: IndexMap<String, Table>,
    pub system_table: Table,
}

impl Database {
    pub fn new(file_path: &str) -> Result<Self, Error> {
        let pager = Rc::new(RefCell::new(Pager::new(file_path)?));
        let tables: IndexMap<String, Table> = IndexMap::new();

        // Borrow in a block so it's released before we pass pager to Table
        let header = {
            let mut p = pager.borrow_mut();
            let page = p.get_page(0)?;
            DatabaseHeader::deserialize(&page.data)
        };

        match header {
            Some(_header) => {
                // Load system tables
                todo!();
            }
            None => {
                let system_table =
                    Table::from_columns("_system".into(), IndexMap::new(), pager.clone())?;

                // Borrow again to write header
                {
                    let mut p = pager.borrow_mut();
                    let page = p.get_page(0)?;
                    let root = system_table.root_page.get();
                    let new_header = DatabaseHeader::new(root);
                    new_header.serialize(root, &mut page.data)?;
                    p.sync()?;
                }

                Ok(Self {
                    pager,
                    tables,
                    system_table,
                })
            }
        }
    }

    /// In-memory database for testing (no file backing)
    pub fn memory() -> Result<Self, Error> {
        let pager = Rc::new(RefCell::new(Pager::memory()));

        Ok(Self {
            pager: pager.clone(),
            tables: IndexMap::new(),
            system_table: Table::from_columns("_system".into(), IndexMap::new(), pager.clone())?,
        })
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
