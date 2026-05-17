use super::table::Table;
use crate::error::Error;
use indexmap::IndexMap;

#[derive(Debug, Default)]
pub struct Database<'a> {
    pub tables: IndexMap<String, &'a Table>,
}

impl<'a> Database<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_table(&self, name: &str) -> Option<&'a Table> {
        self.tables.get(name).copied()
    }

    pub fn table_exists(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    pub fn add_table(&mut self, table: &'a Table) -> Result<(), Error> {
        if self.table_exists(&table.name) {
            return Err(Error::DuplicateTable(table.name.clone()));
        }
        self.tables.insert(table.name.clone(), table);

        Ok(())
    }
}
