use super::pager::Pager;
use super::table::{Table, TableBuilder};
use crate::error::Error;
use crate::magic::DatabaseHeader;
use crate::types::{Column, ColumnType, Type, Value};
use indexmap::IndexMap;
use log::debug;
use ordered_float::OrderedFloat;
use std::cell::RefCell;
use std::rc::Rc;

// System table columns
fn system_table_columns() -> IndexMap<String, Column> {
    let mut cols = IndexMap::new();
    cols.insert(
        "table_name".into(),
        Column::new("table_name".into(), ColumnType::Varchar(64)),
    );

    cols.insert(
        "root_page".into(),
        Column::new("root_page".into(), ColumnType::Number),
    );

    cols.insert(
        "columns".into(),
        Column::new("columns".into(), ColumnType::Varchar(1024)),
    ); //@TODO: this is a little
    //hacky, maybe this should be a key into another table or something.

    cols
}

#[derive(Debug)]
pub struct Database {
    pub pager: Rc<RefCell<Pager>>,
    pub tables: IndexMap<String, Table>,
    pub system_table: Table,
    pub next_row_id: usize,
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

                // Load system tables
                let system_table = Table::load(
                    "_system_table".into(),
                    system_table_columns(),
                    header.system_table_page,
                    pager.clone(),
                );

                debug!("Loaded system table: {:?}", system_table);

                let mut cursor = system_table.start()?;
                debug!("System table cursor: {:?}", cursor);

                let mut n_rows = 0;
                while !cursor.eot {
                    debug!(
                        "System table entry at page={} cell={}",
                        cursor.page_num, cursor.cell_num,
                    );

                    let row = cursor.row()?;
                    debug!("Row: {:?}", row);
                    let Value::Varchar(table_name) = row[1].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: Type::Varchar(64),
                            got: row[1].typ(),
                        });
                    };
                    let Value::Number(n) = row[2].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: Type::Number,
                            got: row[2].typ(),
                        });
                    };
                    let table_page_num = n.into_inner() as usize;

                    let Value::Varchar(stringified_columns) = row[3].clone() else {
                        return Err(Error::TypeMismatch {
                            expected: Type::Varchar(1024),
                            got: row[3].typ(),
                        });
                    };

                    let mut user_columns: IndexMap<String, Column> = IndexMap::new();

                    // Now we want to create the user columns, each cell represents one
                    for column in stringified_columns.split(';') {
                        let (name, type_str) = column
                            .split_once(':')
                            .ok_or(Error::CorruptedTree("Invalid column format".into()))?;

                        let column_type =
                            ColumnType::from_str(type_str).ok_or(Error::CorruptedTree(format!(
                                "Invalid Type found in system table: {:?}",
                                type_str
                            )))?;
                        user_columns
                            .insert(name.to_string(), Column::new(name.to_string(), column_type));
                    }

                    let table = Table::load(
                        table_name.clone(),
                        user_columns,
                        table_page_num,
                        pager.clone(),
                    );
                    debug!("Loaded table: {:?}", table.name);
                    tables.insert(table_name, table);
                    n_rows += 1;
                    cursor.advance()?;
                }

                Ok(Self {
                    pager,
                    tables,
                    system_table,
                    next_row_id: n_rows,
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
                )?;

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
                    next_row_id: 0,
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
            )?,
            next_row_id: 0,
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

        // Insert the tables metadata into the system table
        let meta = table.metadata_as_values()?;
        let rowid = Value::Number(OrderedFloat(self.next_row_id as f64));
        self.next_row_id += 1;

        let row = vec![
            rowid.clone(),
            meta.name,
            meta.root_page,
            meta.columns,
            meta.primary_key_index,
        ];

        self.system_table.insert(&rowid, &row)?;

        // Insert it into the database tables
        self.tables.insert(table.name.clone(), table);

        Ok(())
    }
}
