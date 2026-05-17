use super::pager::{PAGE_SIZE, Pager};
use crate::column::{Column, ColumnType, deserialize_row, serialize_row};
use crate::cursor::Cursor;
use crate::error::Error;
use crate::node::{HEADER_SIZE, Node};
use crate::value::Value;
use indexmap::IndexMap;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;

#[derive(Debug)]
pub struct TableBuilder {
    name: String,
    columns: IndexMap<String, Column>,
    error: Option<Error>,
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

    pub fn build(self) -> Result<Table, Error> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if self.columns.is_empty() {
            return Err(Error::NoColumns);
        }
        Ok(Table::from_columns(self.name, self.columns))
    }
}

#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub columns: IndexMap<String, Column>,
    pub rows: Cell<usize>,
    pub root_page: Cell<usize>, // Page number of the root node
    pub row_size: usize,
    pub rows_per_page: usize,
    pub primary_key_name: String,
    pub primary_key_index: usize,
    pub key_size: usize,
    pub cell_size: usize,
    pub max_cells_per_leaf: usize,
    pub max_keys_per_internal: usize,
    pub pager: RefCell<Pager>,
}

impl Table {
    pub fn new(name: String, columns: impl IntoIterator<Item = (String, ColumnType)>) -> Self {
        let columns: IndexMap<String, Column> = columns
            .into_iter()
            .map(|(name, column_type)| (name.clone(), Column::new(name, column_type)))
            .collect();

        Self::from_columns(name, columns)
    }

    pub fn get_column(&self, column_name: &str) -> Option<&Column> {
        self.columns.get(column_name)
    }

    pub fn key_column(&self) -> &Column {
        self.columns.get_index(self.primary_key_index).unwrap().1
    }

    pub fn start<'a>(&'a self) -> Cursor<'a> {
        let mut page_num = self.root_page.get();
        let mut pager = self.pager.borrow_mut();

        loop {
            let page = pager.get_page(page_num);

            if Node::read_is_leaf(&page.data) {
                let num_cells = Node::read_num_cells(&page.data);
                return Cursor {
                    table: self,
                    eot: num_cells == 0,
                    page_num,
                    cell_num: 0,
                };
            } else {
                page_num = Node::read_left_child(&page.data);
            }
        }
    }
    pub fn end<'a>(&'a self) -> Cursor<'a> {
        let mut page_num = self.root_page.get();
        let mut pager = self.pager.borrow_mut();

        loop {
            let page = pager.get_page(page_num);

            if Node::read_is_leaf(&page.data) {
                let num_cells = Node::read_num_cells(&page.data);
                return Cursor {
                    table: self,
                    eot: true,
                    page_num,
                    cell_num: num_cells,
                };
            } else {
                page_num = Node::read_right_child(&page.data);
            }
        }
    }

    pub fn search(&self, key: &Value) -> Result<(Cursor<'_>, bool), Error> {
        let leaf_page = self.leaf_search(self.root_page.get(), key)?;
        let node = self.read_node(leaf_page);
        let (cell_num, found) = self.cell_search(leaf_page, key)?;
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

    pub fn cell_search(&self, page_num: usize, key: &Value) -> Result<(usize, bool), Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num);
        if !Node::read_is_leaf(&page.data) {
            Err(Error::WrongNodeType(
                "cell_search called on a non-leaf node".into(),
            ))
        } else {
            let num_cells = Node::read_num_cells(&page.data);
            let mut lo = 0;
            let mut hi = num_cells;

            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                let mid_key = Node::read_key_at(&page.data, mid, self.key_column(), self.row_size)?;

                match mid_key.cmp(key) {
                    Ordering::Less => lo = mid + 1,
                    Ordering::Greater => hi = mid,
                    Ordering::Equal => return Ok((mid, true)),
                }
            }

            Ok((lo, false)) // insertion point                
        }
    }

    pub fn leaf_search(&self, page_num: usize, key: &Value) -> Result<usize, Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num);
        if Node::read_is_leaf(&page.data) {
            Ok(page_num)
        } else {
            let m = Node::read_num_cells(&page.data) + 1;
            for i in 0..m {
                let node_key = Node::read_key_at(&page.data, i, self.key_column(), self.row_size)?;

                if key.leq(&node_key) {
                    let child = Node::read_child_at(&page.data, i, self.key_column())?;
                    return self.leaf_search(child, key);
                }
            }

            let child = Node::read_child_at(&page.data, m - 1, self.key_column())?;
            self.leaf_search(child, key)
        }
    }

    pub fn read_node(&self, page_num: usize) -> Node {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num);
        let cols: Vec<&Column> = self.columns.values().collect();
        Node::deserialize(&page.data, self.key_size, self.row_size, &cols)
    }

    pub fn write_node(&self, page_num: usize, node: &Node) {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num);
        let cols: Vec<&Column> = self.columns.values().collect();
        node.serialize(&mut page.data, self.key_size, self.row_size, &cols);
    }

    pub fn with_node<F, R>(&self, page_num: usize, f: F) -> R
    where
        F: FnOnce(&Node) -> R,
    {
        let node = self.read_node(page_num);
        f(&node)
    }

    pub fn with_node_mut<F, R>(&self, page_num: usize, f: F) -> R
    where
        F: FnOnce(&mut Node) -> R,
    {
        let mut node = self.read_node(page_num);
        let result = f(&mut node);
        self.write_node(page_num, &node);
        result
    }

    pub fn insert_into_leaf(&self, page_num: usize, key: &Value, row: &[Value]) {
        let node = self.read_node(page_num);
        let Node::Leaf { cells, .. } = node else {
            panic!("insert_into_leaf called on non-leaf");
        };

        // Find insert position
        let cell_num = match cells.binary_search_by(|(k, _)| k.cmp(key)) {
            Ok(idx) => idx,  // duplicate key - overwrite
            Err(idx) => idx, // insert position
        };

        self.shift_cells_right(page_num, cell_num);
        self.write_cell(page_num, cell_num, key, row);
    }

    pub fn insert_into_internal(&self, page_num: usize, split_key: &Value, new_child: usize) {
        let mut node = self.read_node(page_num);
        let Node::Internal {
            ref mut keys,
            ref mut children,
            ..
        } = node
        else {
            panic!("insert_into_internal called on non-internal");
        };

        // Find insert position for key
        let idx = match keys.binary_search(split_key) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        };

        keys.insert(idx, split_key.clone());
        children.insert(idx + 1, new_child);

        self.write_node(page_num, &node);
    }

    pub fn split_leaf(&self, page_num: usize) -> (Value, usize) {
        let mut node = self.read_node(page_num);
        let Node::Leaf {
            ref mut cells,
            ref mut next_leaf,
            parent,
        } = node
        else {
            panic!("split_leaf called on non-leaf");
        };

        let mid = cells.len() / 2;
        let right_cells: Vec<_> = cells.drain(mid..).collect();
        let split_key = right_cells[0].0.clone();

        // Allocate new page for right sibling
        let new_page = self.pager.borrow_mut().alloc_page();

        // Right sibling points to old next_leaf
        let right_node = Node::Leaf {
            parent,
            cells: right_cells,
            next_leaf: *next_leaf,
        };

        // Left node now points to right sibling
        *next_leaf = Some(new_page);

        self.write_node(page_num, &node);
        self.write_node(new_page, &right_node);

        (split_key, new_page)
    }

    pub fn split_internal(&self, page_num: usize) -> (Value, usize) {
        let mut node = self.read_node(page_num);
        let Node::Internal {
            ref mut keys,
            ref mut children,
            parent,
        } = node
        else {
            panic!("split_internal called on non-internal");
        };

        let mid = keys.len() / 2;

        // Middle key goes UP to parent, not into either half
        let split_key = keys[mid].clone();

        // Right half gets keys after mid, and children after mid
        let right_keys: Vec<_> = keys.drain(mid + 1..).collect();
        let right_children: Vec<_> = children.drain(mid + 1..).collect();

        // Remove the middle key from left half (it goes up)
        keys.pop();

        let new_page = self.pager.borrow_mut().alloc_page();

        let right_node = Node::Internal {
            parent,
            keys: right_keys,
            children: right_children.clone(),
        };

        self.write_node(page_num, &node);
        self.write_node(new_page, &right_node);

        // Update parent pointers for children that moved to the new sibling
        for &child_page in &right_children {
            self.set_parent(child_page, new_page);
        }

        (split_key, new_page)
    }

    pub fn split_leaf_and_insert(
        &self,
        page_num: usize,
        key: &Value,
        row: &[Value],
    ) -> (Value, usize) {
        let (split_key, new_page) = self.split_leaf(page_num);

        if key.leq(&split_key) {
            self.insert_into_leaf(page_num, key, row);
        } else {
            self.insert_into_leaf(new_page, key, row);
        }

        (split_key, new_page)
    }

    pub fn split_internal_and_insert(
        &self,
        page_num: usize,
        key: &Value,
        new_child: usize,
    ) -> (Value, usize) {
        let (split_key, new_page) = self.split_internal(page_num);

        if key.leq(&split_key) {
            self.insert_into_internal(page_num, key, new_child);
        } else {
            self.insert_into_internal(new_page, key, new_child);
        }

        (split_key, new_page)
    }

    pub fn node_is_full(&self, page_num: usize) -> bool {
        //@TODO can probably be optimized to not read the whole node...
        let node = self.read_node(page_num);
        match node {
            Node::Leaf { cells, .. } => self.max_cells_per_leaf <= cells.len(),
            Node::Internal { keys, .. } => self.max_keys_per_internal <= keys.len(),
        }
    }

    pub fn insert_recursive(
        &self,
        page_num: usize,
        key: &Value,
        values: &[Value],
    ) -> Option<(Value, usize)> {
        let node = self.read_node(page_num);

        match node {
            Node::Internal { keys, children, .. } => {
                let child_idx = match keys.binary_search(key) {
                    Ok(idx) => idx + 1, // key found, go right
                    Err(idx) => idx,    // go to child at insert position
                };

                if let Some((split_key, new_sibling)) =
                    self.insert_recursive(children[child_idx], key, values)
                {
                    if keys.len() < self.max_keys_per_internal {
                        self.insert_into_internal(page_num, &split_key, new_sibling);
                        None
                    } else {
                        Some(self.split_internal_and_insert(page_num, &split_key, new_sibling))
                    }
                } else {
                    None
                }
            }
            Node::Leaf { cells, .. } => {
                if cells.len() < self.max_cells_per_leaf {
                    self.insert_into_leaf(page_num, key, values);
                    None
                } else {
                    let (split_key, new_sibling) =
                        self.split_leaf_and_insert(page_num, key, values);
                    Some((split_key, new_sibling))
                }
            }
        }
    }
    pub fn insert(&self, key: &Value, row: &[Value]) {
        let old_root = self.root_page.get();
        if let Some((split_key, new_sibling)) = self.insert_recursive(old_root, key, row) {
            self.create_new_root(&split_key, old_root, new_sibling);
        }
    }

    pub fn delete(&self, key: &Value) -> Result<(), Error> {
        //@TODO thsi is the simple approach of not actually doing the re-balancing,
        //as even without re-balancing we still get the logarithmic search property,
        //it is not uncommon to just naively delete
        let (cursor, found) = self.search(key)?;
        if found {
            cursor.with_node_mut(|mut node| {
                if let Node::Leaf { cells, .. } = &mut node {
                    cells.remove(cursor.cell_num);
                };
            });
        }

        Ok(())
    }

    pub fn create_new_root(&self, split_key: &Value, left_child: usize, right_child: usize) {
        let new_root_page = self.pager.borrow_mut().alloc_page();

        let new_root = Node::Internal {
            parent: None,
            keys: vec![split_key.clone()],
            children: vec![left_child, right_child],
        };

        self.write_node(new_root_page, &new_root);
        self.root_page.set(new_root_page);

        // Update children's parent pointers
        self.set_parent(left_child, new_root_page);
        self.set_parent(right_child, new_root_page);
    }

    fn set_parent(&self, page_num: usize, parent_page: usize) {
        let mut node = self.read_node(page_num);
        match &mut node {
            Node::Internal { parent, .. } | Node::Leaf { parent, .. } => {
                *parent = Some(parent_page);
            }
        }
        self.write_node(page_num, &node);
    }

    pub fn shift_cells_right(&self, page_num: usize, from: usize) {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num);

        let num_cells = Node::read_num_cells(&page.data);
        let shift_start = HEADER_SIZE + from * self.cell_size;
        let shift_end = HEADER_SIZE + num_cells * self.cell_size;

        page.data
            .copy_within(shift_start..shift_end, shift_start + self.cell_size);

        let new_count = (num_cells + 1) as u16;
        page.data[6..8].copy_from_slice(&new_count.to_le_bytes());
    }

    pub fn write_cell(&self, page_num: usize, cell_num: usize, key: &Value, row: &[Value]) {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num);

        let offset = HEADER_SIZE + cell_num * self.cell_size;
        key.serialize(&mut page.data[offset..], self.key_size);

        let cols: Vec<&Column> = self.columns.values().collect();
        serialize_row(row, cols, &mut page.data[offset + self.key_size..]);
    }

    fn from_columns(name: String, user_columns: IndexMap<String, Column>) -> Self {
        // Prepend built-in columns
        let mut columns = IndexMap::new();
        columns.insert(
            "_rowid".to_string(),
            Column::new("_rowid".to_string(), ColumnType::Number),
        );
        columns.extend(user_columns);

        let row_size: usize = columns.iter().map(|(_, c)| c.column_size).sum();
        let rows_per_page = PAGE_SIZE / row_size;
        let primary_key_name = columns[0].name.clone();
        let key_size = columns[0].column_size;
        let cell_size = key_size + row_size;
        let max_cells_per_leaf = (PAGE_SIZE - HEADER_SIZE) / cell_size;
        let max_keys_per_internal = (PAGE_SIZE - HEADER_SIZE) / (4 + key_size);

        let pager = RefCell::new(Pager::new());
        // Initialize root as empty leaf
        let root_page = pager.borrow_mut().alloc_page();
        let cols: Vec<&Column> = columns.values().collect();
        let empty_leaf = Node::Leaf {
            parent: None,
            cells: vec![],
            next_leaf: None,
        };
        // We need to serialize it to the page
        empty_leaf.serialize(
            &mut pager.borrow_mut().get_page(root_page).data,
            key_size,
            row_size,
            &cols,
        );
        Table {
            name,
            columns,
            rows: Cell::new(0),
            row_size,
            root_page: Cell::new(root_page),
            rows_per_page,
            primary_key_name,
            primary_key_index: 0,
            key_size,
            cell_size,
            max_cells_per_leaf,
            max_keys_per_internal,
            pager,
        }
    }

    /// Returns only user-defined columns (excludes system columns like _rowid)
    pub fn user_columns(&self) -> impl Iterator<Item = &Column> {
        self.columns.values().filter(|c| !c.name.starts_with('_'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ordered_float::OrderedFloat;

    fn make_test_table() -> Table {
        TableBuilder::new("test")
            .column("name", ColumnType::Varchar(32))
            .column("age", ColumnType::Number)
            .build()
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
            .build();
        assert!(matches!(result, Err(Error::ReservedColumnName(_))));
    }

    #[test]
    fn test_table_builder_duplicate_column() {
        let result = TableBuilder::new("test")
            .column("name", ColumnType::Varchar(32))
            .column("name", ColumnType::Number)
            .build();
        assert!(matches!(result, Err(Error::DuplicateColumn(_))));
    }

    #[test]
    fn test_table_builder_no_columns() {
        let result = TableBuilder::new("test").build();
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
        table.write_node(0, &empty_leaf);

        let cursor = table.start();
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
        table.write_node(0, &empty_leaf);

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
        table.write_node(0, &leaf);

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
