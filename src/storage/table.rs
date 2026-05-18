use super::btree::BTree;
use super::cursor::Cursor;
use super::node::Node;
use super::pager::Pager;
use crate::error::Error;
use crate::types::{Column, ColumnType, Value};
use indexmap::IndexMap;
use ordered_float::OrderedFloat;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug)]
pub struct TableBuilder {
    name: String,
    columns: IndexMap<String, Column>,
    error: Option<Error>,
}

#[derive(Debug)]
pub struct TableMetadataValues {
    pub name: Value,
    pub root_page: Value,
    pub columns: Value,
    pub primary_key_index: Value,
}

impl TableBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        TableBuilder {
            name: name.into(),
            columns: IndexMap::new(),
            error: None,
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
        if self.columns.contains_key(&name) {
            self.error = Some(Error::DuplicateColumn(name));
            return self;
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
        Table::from_columns(self.name, self.columns, pager)
    }
}

#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub columns: IndexMap<String, Column>,
    pub next_row_id: Cell<usize>,
    pub primary_key_name: String,
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

        Self::from_columns(name, columns, pager)
    }

    pub fn get_column(&self, column_name: &str) -> Option<&Column> {
        self.columns.get(column_name)
    }

    pub fn key_column(&self) -> &Column {
        self.columns.get_index(self.primary_key_index).unwrap().1
    }

    fn cols(&self) -> Vec<&Column> {
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
        let leaf_page = self.btree.leaf_search(self.btree.root_page.get(), key, self.key_column())?;
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
        self.btree.read_node(page_num, &self.cols())
    }

    pub fn write_node(&self, page_num: usize, node: &Node) -> Result<(), Error> {
        self.btree.write_node(page_num, node, &self.cols())
    }

    pub fn with_node<F, R>(&self, page_num: usize, f: F) -> Result<R, Error>
    where
        F: FnOnce(&Node) -> Result<R, Error>,
    {
        self.btree.with_node(page_num, &self.cols(), f)
    }

    pub fn with_node_mut<F, R>(&self, page_num: usize, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Node) -> Result<R, Error>,
    {
        self.btree.with_node_mut(page_num, &self.cols(), f)
    }

    pub fn shift_cells_right(&self, page_num: usize, from: usize) -> Result<(), Error> {
        self.btree.shift_cells_right(page_num, from)
    }

    pub fn write_cell(
        &self,
        page_num: usize,
        cell_num: usize,
        key: &Value,
        row: &[Value],
    ) -> Result<(), Error> {
        self.btree.write_cell(page_num, cell_num, key, row, &self.cols())
    }

    pub fn insert(&self, key: &Value, row: &[Value]) -> Result<(), Error> {
        self.btree.insert(key, row, &self.cols())
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
    ) -> Result<Self, Error> {
        // Prepend built-in columns
        let mut columns = IndexMap::new();
        columns.insert(
            "_rowid".to_string(),
            Column::new("_rowid".to_string(), ColumnType::Number),
        );
        columns.extend(user_columns);

        let row_size: usize = columns.iter().map(|(_, c)| c.column_size).sum();
        let primary_key_name = columns[0].name.clone();
        let key_size = columns[0].column_size;

        let cols: Vec<&Column> = columns.values().collect();
        let btree = BTree::new(row_size, key_size, pager, &cols);

        Ok(Table {
            name,
            columns,
            next_row_id: Cell::new(0),
            primary_key_name,
            primary_key_index: 0,
            btree,
        })
    }

    /// Returns only user-defined columns (excludes system columns like _rowid)
    pub fn user_columns(&self) -> impl Iterator<Item = &Column> {
        self.columns.values().filter(|c| !c.name.starts_with('_'))
    }

    pub fn metadata_as_values(&self) -> Result<TableMetadataValues, Error> {
        let name_as_value = Value::Varchar(self.name.clone());
        let root_page_as_value = Value::Number(OrderedFloat(self.btree.root_page.get() as f64));

        let str_values = self
            .columns
            .iter()
            .map(|(_, value)| value.to_string())
            .collect::<Vec<String>>();
        let combined = str_values.join(";");
        let columns_as_value = Value::Varchar(combined);

        let primary_key_index_as_value = Value::Number(OrderedFloat(self.primary_key_index as f64));
        Ok(TableMetadataValues {
            name: name_as_value,
            columns: columns_as_value,
            root_page: root_page_as_value,
            primary_key_index: primary_key_index_as_value,
        })
    }

    /// Load an existing table from stored metadata (doesn't allocate a new root page)
    pub fn load(
        name: String,
        user_columns: IndexMap<String, Column>,
        root_page: usize,
        pager: Rc<RefCell<Pager>>,
    ) -> Self {
        let mut columns = IndexMap::new();
        columns.insert(
            "_rowid".to_string(),
            Column::new("_rowid".to_string(), ColumnType::Number),
        );
        columns.extend(user_columns);

        let row_size: usize = columns.iter().map(|(_, c)| c.column_size).sum();
        let primary_key_name = columns[0].name.clone();
        let key_size = columns[0].column_size;

        let btree = BTree::load(root_page, row_size, key_size, pager);

        Table {
            name,
            columns,
            next_row_id: Cell::new(0),
            primary_key_name,
            primary_key_index: 0,
            btree,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{deserialize_row, serialize_row};
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

    #[test]
    fn test_table_builder() {
        let table = make_test_table();
        assert_eq!(table.name, "test");
        assert_eq!(table.columns.len(), 3); // _rowid + name + age
        assert_eq!(table.primary_key_name, "_rowid");
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
    fn test_serialize_deserialize_row() {
        let cols = vec![
            Column::new("id".into(), ColumnType::Number),
            Column::new("name".into(), ColumnType::Varchar(8)),
            Column::new("active".into(), ColumnType::Bool),
        ];
        let col_refs: Vec<&Column> = cols.iter().collect();

        let values = vec![num(42.0), Value::Varchar("hello".into()), Value::Bool(true)];

        let mut buf = vec![0u8; 17]; // 8 + 8 + 1
        serialize_row(&values, col_refs.clone(), &mut buf);

        let result = deserialize_row(&col_refs, &buf);
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
        let leaf = Node::Leaf {
            parent: None,
            cells: vec![
                (
                    num(1.0),
                    vec![num(1.0), Value::Varchar("alice".into()), num(30.0)],
                ),
                (
                    num(2.0),
                    vec![num(2.0), Value::Varchar("bob".into()), num(25.0)],
                ),
                (
                    num(3.0),
                    vec![num(3.0), Value::Varchar("carol".into()), num(35.0)],
                ),
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
