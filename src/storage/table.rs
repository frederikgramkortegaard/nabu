use super::btree::BTree;
use super::cursor::Cursor;
use super::node::Node;
use super::pager::Pager;
use crate::constants::{MAX_COLUMNS, MAX_COLUMNS_STR_LEN, MAX_TABLE_NAME_LEN, MAX_VARCHAR_LEN, NULL_BITMAP_SIZE};
use crate::error::Error;
use crate::types::{Column, ColumnType, Row, Value};
use indexmap::IndexMap;
use ordered_float::OrderedFloat;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// System columns prepended to every user table
fn system_columns() -> IndexMap<String, Column> {
    IndexMap::from([
        (
            "_rowid".to_string(),
            Column::new("_rowid".to_string(), ColumnType::Number),
        ),
        (
            "_tidmin".to_string(),
            Column::new("_tidmin".to_string(), ColumnType::Number),
        ),
        (
            "_tidmax".to_string(),
            Column::new("_tidmax".to_string(), ColumnType::Number),
        ),
        (
            "_null_bitmap".to_string(),
            Column::new("_null_bitmap".to_string(), ColumnType::Bytes(NULL_BITMAP_SIZE)),
        ),
    ])
}

#[derive(Debug)]
pub struct TableBuilder {
    name: String,
    columns: IndexMap<String, Column>,
    error: Option<Error>,
}

#[derive(Debug)]
pub struct TableMetadata {
    pub name: String,
    pub root_page: usize,
    pub user_columns: IndexMap<String, Column>,
    pub primary_key_index: usize,
}

impl TableMetadata {
    pub fn new(table: &Table) -> Self {
        TableMetadata {
            name: table.name.clone(),
            root_page: table.btree.root_page.get(),
            user_columns: table
                .user_columns()
                .map(|c| (c.name.clone(), c.clone()))
                .collect(),
            primary_key_index: table.primary_key_index,
        }
    }

    /// Convert to a Row for storing in system table
    pub fn to_row(&self) -> Row {
        assert!(self.name.len() <= MAX_TABLE_NAME_LEN);

        let columns_str = self
            .user_columns
            .values()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(";");

        assert!(columns_str.len() <= MAX_COLUMNS_STR_LEN);

        Row(vec![
            Value::Varchar(self.name.clone()),
            Value::Number(OrderedFloat(self.root_page as f64)),
            Value::Varchar(columns_str),
            Value::Number(OrderedFloat(self.primary_key_index as f64)),
        ])
    }

    /// Parse from a Row read from system table
    pub fn from_row(row: &Row) -> Result<Self, Error> {
        let Value::Varchar(name) = &row[0] else {
            return Err(Error::CorruptedTree("expected name as varchar".into()));
        };

        let Value::Number(root_page) = &row[1] else {
            return Err(Error::CorruptedTree("expected root_page as number".into()));
        };

        let Value::Varchar(columns_str) = &row[2] else {
            return Err(Error::CorruptedTree("expected columns as varchar".into()));
        };

        let Value::Number(primary_key_index) = &row[3] else {
            return Err(Error::CorruptedTree(
                "expected primary_key_index as number".into(),
            ));
        };

        let mut user_columns = IndexMap::new();
        for col_str in columns_str.split(';') {
            let col: Column = col_str.parse().map_err(|e| Error::CorruptedTree(e))?;
            user_columns.insert(col.name.clone(), col);
        }

        Ok(TableMetadata {
            name: name.clone(),
            root_page: root_page.into_inner() as usize,
            user_columns,
            primary_key_index: primary_key_index.into_inner() as usize,
        })
    }
}

impl TableBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let error = if name.len() > MAX_TABLE_NAME_LEN {
            Some(Error::TableNameTooLong(name.len()))
        } else {
            None
        };
        TableBuilder {
            name,
            columns: IndexMap::new(),
            error,
        }
    }

    pub fn column(mut self, name: impl Into<String>, column_type: ColumnType) -> Self {
        if self.error.is_some() {
            return self;
        }
        let name = name.into();
        if name.starts_with('_') {
            self.error = Some(Error::ReservedColumnName(name));
            return self;
        }
        if let Some(ch) = name.chars().find(|c| c.is_ascii_punctuation() && *c != '_') {
            self.error = Some(Error::InvalidColumnName {
                name,
                reason: format!("contains invalid character '{}'", ch),
            });
            return self;
        }
        if self.columns.contains_key(&name) {
            self.error = Some(Error::DuplicateColumn(name));
            return self;
        }
        if let ColumnType::Varchar(len) = column_type {
            if len > MAX_VARCHAR_LEN {
                self.error = Some(Error::VarcharTooLong {
                    max: MAX_VARCHAR_LEN,
                    got: len,
                });
                return self;
            }
        }
        self.columns
            .insert(name.clone(), Column::new(name, column_type));
        self
    }

    pub fn build(self, pager: Rc<RefCell<Pager>>) -> Result<Table, Error> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if self.columns.is_empty() {
            return Err(Error::NoColumns);
        }
        let columns_str_len: usize = self
            .columns
            .values()
            .map(|c| c.to_string().len())
            .sum::<usize>()
            + self.columns.len().saturating_sub(1); // account for ';' separators
        if columns_str_len > MAX_COLUMNS_STR_LEN {
            return Err(Error::ColumnsTooLong(columns_str_len));
        }
        Table::from_columns(self.name, self.columns, pager, false)
    }
}

#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub columns: IndexMap<String, Column>,
    pub next_row_id: Cell<usize>,
    pub primary_key_index: usize,
    pub btree: BTree,
}

impl Table {
    pub fn new(
        name: String,
        columns: impl IntoIterator<Item = (String, ColumnType)>,
        pager: Rc<RefCell<Pager>>,
    ) -> Result<Self, Error> {
        let columns: IndexMap<String, Column> = columns
            .into_iter()
            .map(|(name, column_type)| (name.clone(), Column::new(name, column_type)))
            .collect();

        Self::from_columns(name, columns, pager, false)
    }

    pub fn get_column(&self, column_name: &str) -> Option<&Column> {
        self.columns.get(column_name)
    }

    pub fn get_user_column(&self, column_name: &str) -> Option<&Column> {
        if column_name.starts_with('_') {
            return None;
        }
        self.columns.get(column_name)
    }

    pub fn key_column(&self) -> &Column {
        self.columns.get_index(self.primary_key_index).unwrap().1
    }

    fn columns(&self) -> Vec<&Column> {
        self.columns.values().collect()
    }

    pub fn pager(&self) -> &Rc<RefCell<Pager>> {
        &self.btree.pager
    }

    pub fn root_page(&self) -> usize {
        self.btree.root_page.get()
    }

    pub fn start(&self) -> Result<Cursor<'_>, Error> {
        let mut page_num = self.btree.root_page.get();
        let mut pager = self.btree.pager.borrow_mut();

        loop {
            let page = pager.get_page(page_num)?;

            if Node::read_is_leaf(&page.data) {
                let num_cells = Node::read_num_cells(&page.data);
                return Ok(Cursor {
                    table: self,
                    eot: num_cells == 0,
                    page_num,
                    cell_num: 0,
                });
            } else {
                page_num = Node::read_left_child(&page.data);
            }
        }
    }

    pub fn end(&self) -> Result<Cursor<'_>, Error> {
        let mut page_num = self.btree.root_page.get();
        let mut pager = self.btree.pager.borrow_mut();

        loop {
            let page = pager.get_page(page_num)?;

            if Node::read_is_leaf(&page.data) {
                let num_cells = Node::read_num_cells(&page.data);
                return Ok(Cursor {
                    table: self,
                    eot: true,
                    page_num,
                    cell_num: num_cells,
                });
            } else {
                page_num = Node::read_right_child(&page.data);
            }
        }
    }

    pub fn search(&self, key: &Value) -> Result<(Cursor<'_>, bool), Error> {
        let leaf_page =
            self.btree
                .leaf_search(self.btree.root_page.get(), key, self.key_column())?;
        let node = self.read_node(leaf_page)?;
        let (cell_num, found) = self.btree.cell_search(leaf_page, key, self.key_column())?;
        let eot = node.next_leaf().is_none() && cell_num >= node.num_cells();

        Ok((
            Cursor {
                table: self,
                eot,
                page_num: leaf_page,
                cell_num,
            },
            found,
        ))
    }

    // Delegate to BTree
    pub fn read_node(&self, page_num: usize) -> Result<Node, Error> {
        self.btree.read_node(page_num, &self.columns())
    }

    pub fn write_node(&self, page_num: usize, node: &Node) -> Result<(), Error> {
        self.btree.write_node(page_num, node, &self.columns())
    }

    pub fn with_node<F, R>(&self, page_num: usize, f: F) -> Result<R, Error>
    where
        F: FnOnce(&Node) -> Result<R, Error>,
    {
        self.btree.with_node(page_num, &self.columns(), f)
    }

    pub fn with_node_mut<F, R>(&self, page_num: usize, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Node) -> Result<R, Error>,
    {
        self.btree.with_node_mut(page_num, &self.columns(), f)
    }

    pub fn insert(&self, key: &Value, row: &Row) -> Result<(), Error> {
        self.btree.insert(key, row, &self.columns())
    }

    pub fn delete(&self, key: &Value) -> Result<(), Error> {
        let (cursor, found) = self.search(key)?;
        if found {
            cursor.with_node_mut(|node| {
                if let Node::Leaf { cells, .. } = node {
                    cells.remove(cursor.cell_num);
                };
                Ok(())
            })?;
        }

        Ok(())
    }

    pub fn from_columns(
        name: String,
        user_columns: IndexMap<String, Column>,
        pager: Rc<RefCell<Pager>>,
        clean: bool,
    ) -> Result<Self, Error> {
        let columns = if clean {
            user_columns
        } else {
            let mut cols = system_columns();
            cols.extend(user_columns);
            cols
        };

        let row_size: usize = columns.iter().map(|(_, c)| c.column_size).sum();
        let key_size = columns[0].column_size;

        let cols: Vec<&Column> = columns.values().collect();
        let btree = BTree::new(row_size, key_size, pager, &cols);

        Ok(Table {
            name,
            columns,
            next_row_id: Cell::new(0),
            primary_key_index: 0,
            btree,
        })
    }

    /// Returns only user-defined columns (excludes system columns)
    pub fn user_columns(&self) -> impl Iterator<Item = &Column> {
        self.columns.values().skip(system_columns().len())
    }

    pub fn load(
        name: String,
        user_columns: IndexMap<String, Column>,
        root_page: usize,
        pager: Rc<RefCell<Pager>>,
        clean: bool,
    ) -> Self {
        let columns = if clean {
            user_columns
        } else {
            let mut cols = system_columns();
            cols.extend(user_columns);
            cols
        };

        let row_size: usize = columns.iter().map(|(_, c)| c.column_size).sum();
        let key_size = columns[0].column_size;

        let btree = BTree::load(root_page, row_size, key_size, pager);

        Table {
            name,
            columns,
            next_row_id: Cell::new(0),
            primary_key_index: 0,
            btree,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ordered_float::OrderedFloat;

    fn make_pager() -> Rc<RefCell<Pager>> {
        Rc::new(RefCell::new(Pager::memory()))
    }

    fn make_test_table() -> Table {
        TableBuilder::new("test")
            .column("name", ColumnType::Varchar(32))
            .column("age", ColumnType::Number)
            .build(make_pager())
            .unwrap()
    }

    fn num(n: f64) -> Value {
        Value::Number(OrderedFloat(n))
    }

    fn system_values() -> Vec<Value> {
        vec![
            num(0.0), // _rowid (will be overwritten)
            num(0.0), // _tidmin
            num(0.0), // _tidmax
            Value::Bytes(vec![0u8; crate::constants::NULL_BITMAP_SIZE]), // _null_bitmap
        ]
    }

    #[test]
    fn test_table_builder() {
        let table = make_test_table();
        assert_eq!(table.name, "test");
        // _rowid + _tidmin + _tidmax + _col_count + _null_bitmap + name + age = 7
        assert_eq!(table.columns.len(), 6); // 4 system + 2 user
        assert_eq!(table.key_column().name, "_rowid");
    }

    #[test]
    fn test_table_builder_reserved_column() {
        let result = TableBuilder::new("test")
            .column("_secret", ColumnType::Number)
            .build(make_pager());
        assert!(matches!(result, Err(Error::ReservedColumnName(_))));
    }

    #[test]
    fn test_table_builder_duplicate_column() {
        let result = TableBuilder::new("test")
            .column("name", ColumnType::Varchar(32))
            .column("name", ColumnType::Number)
            .build(make_pager());
        assert!(matches!(result, Err(Error::DuplicateColumn(_))));
    }

    #[test]
    fn test_table_builder_no_columns() {
        let result = TableBuilder::new("test").build(make_pager());
        assert!(matches!(result, Err(Error::NoColumns)));
    }

    #[test]
    fn test_table_builder_invalid_column_name() {
        // is_ascii_punctuation covers: !"#$%&'()*+,-./:;<=>?@[\]^_`{|}~
        for invalid_char in [
            '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-', '=', '+', '[', ']', '{', '}',
            '|', '\\', ':', ';', '"', '\'', '<', '>', ',', '.', '/', '?', '`', '~',
        ] {
            let name = format!("col{}name", invalid_char);
            let result = TableBuilder::new("test")
                .column(&name, ColumnType::Number)
                .build(make_pager());
            assert!(
                matches!(result, Err(Error::InvalidColumnName { .. })),
                "expected InvalidColumnName for '{}', got {:?}",
                invalid_char,
                result
            );
        }
    }

    #[test]
    fn test_serialize_deserialize_row() {
        let cols = vec![
            Column::new("id".into(), ColumnType::Number),
            Column::new("name".into(), ColumnType::Varchar(8)),
            Column::new("active".into(), ColumnType::Bool),
        ];
        let col_refs: Vec<&Column> = cols.iter().collect();

        let row = Row(vec![
            num(42.0),
            Value::Varchar("hello".into()),
            Value::Bool(true),
        ]);

        let mut buf = vec![0u8; 17]; // 8 + 8 + 1
        row.serialize(col_refs.clone(), &mut buf);

        let result = Row::deserialize(&col_refs, &buf);
        assert_eq!(result[0], num(42.0));
        assert_eq!(result[1], Value::Varchar("hello".into()));
        assert_eq!(result[2], Value::Bool(true));
    }

    #[test]
    fn test_empty_table_start_is_eot() {
        let table = make_test_table();
        // Write an empty leaf node to page 0
        let empty_leaf = Node::Leaf {
            parent: None,
            cells: vec![],
            next_leaf: None,
        };
        table.write_node(table.root_page(), &empty_leaf).unwrap();

        let cursor = table.start().unwrap();
        assert!(cursor.eot);
        assert_eq!(cursor.cell_num, 0);
    }

    #[test]
    fn test_search_empty_table() {
        let table = make_test_table();
        let empty_leaf = Node::Leaf {
            parent: None,
            cells: vec![],
            next_leaf: None,
        };
        table.write_node(table.root_page(), &empty_leaf).unwrap();

        let (cursor, found) = table.search(&num(1.0)).unwrap();
        assert!(!found);
        assert!(cursor.eot);
    }

    #[test]
    fn test_search_single_leaf() {
        let table = make_test_table();
        // 2 user columns: name, age
        let mut row1 = system_values(); row1[0] = num(1.0);
        row1.extend([Value::Varchar("alice".into()), num(30.0)]);
        let mut row2 = system_values(); row2[0] = num(2.0);
        row2.extend([Value::Varchar("bob".into()), num(25.0)]);
        let mut row3 = system_values(); row3[0] = num(3.0);
        row3.extend([Value::Varchar("carol".into()), num(35.0)]);

        let leaf = Node::Leaf {
            parent: None,
            cells: vec![
                (num(1.0), Row(row1)),
                (num(2.0), Row(row2)),
                (num(3.0), Row(row3)),
            ],
            next_leaf: None,
        };
        table.write_node(table.root_page(), &leaf).unwrap();

        // Search for existing key
        let (cursor, found) = table.search(&num(2.0)).unwrap();
        assert!(found);
        assert_eq!(cursor.cell_num, 1);
        assert!(!cursor.eot);

        // Search for non-existing key (would be inserted at position 1)
        let (cursor, found) = table.search(&num(1.5)).unwrap();
        assert!(!found);
        assert_eq!(cursor.cell_num, 1);

        // Search for key past end
        let (cursor, found) = table.search(&num(10.0)).unwrap();
        assert!(!found);
        assert!(cursor.eot); // last leaf, past all cells
    }
}
