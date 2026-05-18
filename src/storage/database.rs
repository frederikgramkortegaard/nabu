use super::pager::Pager;
use super::table::{Table, TableBuilder, TableMetadata};
use crate::constants::{MAX_COLUMNS_STR_LEN, MAX_TABLE_NAME_LEN};
use crate::error::Error;
use crate::magic::DatabaseHeader;
use crate::types::{Column, ColumnType, Type, Value};
use indexmap::IndexMap;
use log::debug;
use std::cell::RefCell;
use std::rc::Rc;

// System table columns
fn system_table_columns() -> IndexMap<String, Column> {
    IndexMap::from([
        ("table_name".into(), Column::new("table_name".into(), ColumnType::Varchar(MAX_TABLE_NAME_LEN))),
        ("root_page".into(), Column::new("root_page".into(), ColumnType::Number)),
        // @TODO: this is a little hacky, maybe this should be a key into another table or something.
        ("columns".into(), Column::new("columns".into(), ColumnType::Varchar(MAX_COLUMNS_STR_LEN))),
        ("primary_key_index".into(), Column::new("primary_key_index".into(), ColumnType::Number)),
    ])
}

#[derive(Debug)]
pub struct Database {
    pub pager: Rc<RefCell<Pager>>,
    pub tables: IndexMap<String, Table>,
    pub system_table: Table,
}

impl Database {
    pub fn new(file_path: &str) -> Result<Self, Error> {
        let pager = Rc::new(RefCell::new(Pager::new(file_path)?));
        let mut tables: IndexMap<String, Table> = IndexMap::new();

        let header = {
            let mut p = pager.borrow_mut();
            let page = p.get_page(0)?;
            DatabaseHeader::deserialize(&page.data)
        };

        match header {
            Some(header) => {
                debug!("Found valid database header: {:?}", header);

                let system_table = Table::load(
                    "_system_table".into(),
                    system_table_columns(),
                    header.system_table_page,
                    pager.clone(),
                    true,
                );

                debug!("Loaded system table: {:?}", system_table);

                let mut cursor = system_table.start()?;
                debug!("System table cursor: {:?}", cursor);

                while !cursor.eot {
                    debug!(
                        "System table entry at page={} cell={}",
                        cursor.page_num, cursor.cell_num,
                    );

                    let row = cursor.row()?;
                    debug!("Row: {:?}", row);
                    let Value::Varchar(table_name) = row[0].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: Type::Varchar(64),
                            got: row[0].typ(),
                        });
                    };
                    let Value::Number(n) = row[1].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: Type::Number,
                            got: row[1].typ(),
                        });
                    };
                    let table_page_num = n.into_inner() as usize;

                    let Value::Varchar(stringified_columns) = row[2].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: Type::Varchar(1024),
                            got: row[2].typ(),
                        });
                    };

                    let mut user_columns: IndexMap<String, Column> = IndexMap::new();

                    // Now we want to create the user columns, each cell represents one
                    for column in stringified_columns.split(';') {
                        let (name, type_str) = column
                            .split_once(':')
                            .ok_or(Error::CorruptedTree("Invalid column format".into()))?;

                        let column_type: ColumnType = type_str.parse().map_err(|_| {
                            Error::CorruptedTree(format!(
                                "Invalid Type found in system table: {:?}",
                                type_str
                            ))
                        })?;
                        user_columns
                            .insert(name.to_string(), Column::new(name.to_string(), column_type));
                    }

                    let table = Table::load(
                        table_name.clone(),
                        user_columns,
                        table_page_num,
                        pager.clone(),
                        false,
                    );
                    debug!("Loaded table: {:?}", table.name);
                    tables.insert(table_name, table);
                    cursor.advance()?;
                }

                Ok(Self {
                    pager,
                    tables,
                    system_table,
                })
            }
            None => {
                // If we weren't able to load a valid header, we're going to assume that there
                // wasn't one. No corruption-recovery tactics. So we just create a new system
                // table, and write to page 0 again.
                let system_table = Table::from_columns(
                    "_system_table".into(),
                    system_table_columns(),
                    pager.clone(),
                    true,
                )?;

                {
                    let mut p = pager.borrow_mut();
                    let page = p.get_page(0)?;
                    let root = system_table.root_page();
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

    /// In-memory database for testing
    pub fn memory() -> Result<Self, Error> {
        let pager = Rc::new(RefCell::new(Pager::memory()));

        Ok(Self {
            pager: pager.clone(),
            tables: IndexMap::new(),
            system_table: Table::from_columns(
                "_system".into(),
                system_table_columns(),
                pager.clone(),
                true,
            )?,
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

        let meta = TableMetadata::new(&table);
        let row = meta.to_row();
        let key = Value::Varchar(table.name.clone()); // table_name is the key

        self.system_table.insert(&key, &row)?;
        self.pager.borrow_mut().sync()?;

        self.tables.insert(table.name.clone(), table);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ColumnType;
    use std::fs;

    #[test]
    fn test_database_create_and_load() {
        let path = "/tmp/rustdb_test.db";
        let _ = fs::remove_file(path);

        // Create database and table
        {
            let mut db = Database::new(path).unwrap();
            db.create_table(
                TableBuilder::new("users")
                    .column("id", ColumnType::Number)
                    .column("name", ColumnType::Varchar(32)),
            )
            .unwrap();
            assert!(db.table_exists("users"));
            assert_eq!(db.tables.len(), 1);
        }

        // Reload database and verify table persisted
        {
            let db = Database::new(path).unwrap();
            assert!(db.table_exists("users"));
            assert_eq!(db.tables.len(), 1);

            let table = db.get_table("users").unwrap();
            // 4 system columns + 2 user columns
            assert_eq!(table.columns.len(), 6);

            let user_cols: Vec<_> = table.user_columns().collect();
            assert_eq!(user_cols.len(), 2);
            assert_eq!(user_cols[0].name, "id");
            assert_eq!(user_cols[1].name, "name");
        }

        let _ = fs::remove_file(path);
    }
}
