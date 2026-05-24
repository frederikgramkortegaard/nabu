use super::pager::Pager;
use super::table::{Table, TableBuilder, TableMetadata};
use crate::catalog::Catalog;
use crate::constants::{MAX_COLUMNS_STR_LEN, MAX_TABLE_NAME_LEN};
use crate::error::Error;
use crate::provider::TableProvider;
use super::magic::DatabaseHeader;
use crate::shared::{field_with_size, new_table_schema, parse_field, DataType, Field, FieldRef, SchemaRef};
use std::sync::Arc;
use crate::shared::Value;
use indexmap::IndexMap;
use log::debug;
use std::cell::RefCell;
use std::rc::Rc;

// System table columns
fn system_table_columns() -> IndexMap<String, Field> {
    IndexMap::from([
        ("table_name".into(), field_with_size("table_name", DataType::FixedSizeBinary(MAX_TABLE_NAME_LEN as i32), MAX_TABLE_NAME_LEN)),
        ("root_page".into(), field_with_size("root_page", DataType::Float64, 8)),
        // @TODO: this is a little hacky, maybe this should be a key into another table or something.
        ("columns".into(), field_with_size("columns", DataType::FixedSizeBinary(MAX_COLUMNS_STR_LEN as i32), MAX_COLUMNS_STR_LEN)),
        ("primary_key_index".into(), field_with_size("primary_key_index", DataType::Float64, 8)),
    ])
}

#[derive(Debug)]
pub struct Database {
    pub pager: Rc<RefCell<Pager>>,
    pub tables: IndexMap<String, Arc<Table>>,
    pub schemas: IndexMap<String, SchemaRef>,
    pub system_table: Table,
}

impl Catalog for Database {
    fn get_schema(&self, name: &str) -> Option<SchemaRef> {
        self.schemas.get(name).cloned()
    }

    fn get_schemas(&self) -> Vec<SchemaRef> {
        self.schemas.values().cloned().collect()
    }

    fn table(&self, name: &str) -> Option<Arc<dyn TableProvider>> {
        self.tables.get(name).map(|t| t.clone() as Arc<dyn TableProvider>)
    }
}

impl Database {
    pub fn new(file_path: &str) -> Result<Self, Error> {
        let pager = Rc::new(RefCell::new(Pager::new(file_path)?));
        let mut tables: IndexMap<String, Arc<Table>> = IndexMap::new();

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
                    let Value::Utf8(table_name) = row[0].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: DataType::Utf8,
                            got: row[0].data_type(),
                        });
                    };
                    let Value::Float64(n) = row[1].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: DataType::Float64,
                            got: row[1].data_type(),
                        });
                    };
                    let table_page_num = n.into_inner() as usize;

                    let Value::Utf8(stringified_columns) = row[2].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: DataType::Utf8,
                            got: row[2].data_type(),
                        });
                    };

                    let mut user_columns: IndexMap<String, Field> = IndexMap::new();

                    // Now we want to create the user columns, each cell represents one
                    // Format is name:type:size;name:type:size;...
                    for column_str in stringified_columns.split(';') {
                        let col = parse_field(column_str).map_err(|e| {
                            Error::CorruptedTree(format!(
                                "Invalid column in system table: {}",
                                e
                            ))
                        })?;
                        user_columns.insert(col.name().to_string(), col);
                    }

                    let table = Arc::new(Table::load(
                        table_name.clone(),
                        user_columns,
                        table_page_num,
                        pager.clone(),
                        false,
                    ));
                    debug!("Loaded table: {:?}", table.name);
                    tables.insert(table_name, table);
                    cursor.advance()?;
                }

                // Build schemas from tables
                let schemas: IndexMap<String, SchemaRef> = tables
                    .iter()
                    .map(|(name, table)| {
                        let fields: Vec<FieldRef> = table
                            .user_columns()
                            .map(|c| Arc::new(c.clone()))
                            .collect();
                        let schema = new_table_schema(name, fields);
                        (name.clone(), schema)
                    })
                    .collect();

                Ok(Self {
                    pager,
                    tables,
                    schemas,
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
                    schemas: IndexMap::new(),
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
            schemas: IndexMap::new(),
            system_table: Table::from_columns(
                "_system".into(),
                system_table_columns(),
                pager.clone(),
                true,
            )?,
        })
    }

    pub fn get_table(&self, name: &str) -> Option<&Table> {
        self.tables.get(name).map(|t| t.as_ref())
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
        let key = Value::Utf8(table.name.clone()); // table_name is the key

        self.system_table.insert(&key, &row)?;
        self.pager.borrow_mut().sync()?;

        // Create schema for the query layer
        let fields: Vec<FieldRef> = table
            .user_columns()
            .map(|c| Arc::new(c.clone()))
            .collect();
        let schema = new_table_schema(&table.name, fields);
        self.schemas.insert(table.name.clone(), schema);
        self.tables.insert(table.name.clone(), Arc::new(table));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    .column("id", DataType::Float64, 8)
                    .column("name", DataType::Utf8, 64),
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
            assert_eq!(user_cols[0].name(), "id");
            assert_eq!(user_cols[1].name(), "name");
        }

        let _ = fs::remove_file(path);
    }
}
