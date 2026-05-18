use super::node::{HEADER_SIZE, Node};
use super::pager::{PAGE_SIZE, Pager};
use crate::error::Error;
use crate::types::{Column, Row, Value};
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Debug)]
pub struct BTree {
    pub root_page: Cell<usize>,
    pub row_size: usize,
    pub key_size: usize,
    pub cell_size: usize,
    pub max_cells_per_leaf: usize,
    pub max_keys_per_internal: usize,
    pub pager: Rc<RefCell<Pager>>,
}

impl BTree {
    pub fn new(
        row_size: usize,
        key_size: usize,
        pager: Rc<RefCell<Pager>>,
        cols: &[&Column],
    ) -> Self {
        let cell_size = key_size + row_size;
        let max_cells_per_leaf = (PAGE_SIZE - HEADER_SIZE) / cell_size;
        let max_keys_per_internal = (PAGE_SIZE - HEADER_SIZE) / (4 + key_size);

        // Initialize root as empty leaf
        let root_page = pager.borrow_mut().alloc_page();
        let empty_leaf = Node::Leaf {
            parent: None,
            cells: vec![],
            next_leaf: None,
        };

        empty_leaf.serialize(
            &mut pager.borrow_mut().get_page(root_page).unwrap().data,
            key_size,
            row_size,
            cols,
        );

        BTree {
            root_page: Cell::new(root_page),
            row_size,
            key_size,
            cell_size,
            max_cells_per_leaf,
            max_keys_per_internal,
            pager,
        }
    }

    pub fn load(
        root_page: usize,
        row_size: usize,
        key_size: usize,
        pager: Rc<RefCell<Pager>>,
    ) -> Self {
        let cell_size = key_size + row_size;
        let max_cells_per_leaf = (PAGE_SIZE - HEADER_SIZE) / cell_size;
        let max_keys_per_internal = (PAGE_SIZE - HEADER_SIZE) / (4 + key_size);

        BTree {
            root_page: Cell::new(root_page),
            row_size,
            key_size,
            cell_size,
            max_cells_per_leaf,
            max_keys_per_internal,
            pager,
        }
    }

    pub fn read_node(&self, page_num: usize, cols: &[&Column]) -> Result<Node, Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num)?;
        Ok(Node::deserialize(
            &page.data,
            self.key_size,
            self.row_size,
            cols,
        ))
    }

    pub fn write_node(&self, page_num: usize, node: &Node, cols: &[&Column]) -> Result<(), Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num)?;
        node.serialize(&mut page.data, self.key_size, self.row_size, cols);
        Ok(())
    }

    pub fn with_node<F, R>(&self, page_num: usize, cols: &[&Column], f: F) -> Result<R, Error>
    where
        F: FnOnce(&Node) -> Result<R, Error>,
    {
        let node = self.read_node(page_num, cols)?;
        f(&node)
    }

    pub fn with_node_mut<F, R>(&self, page_num: usize, cols: &[&Column], f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Node) -> Result<R, Error>,
    {
        let mut node = self.read_node(page_num, cols)?;
        let result = f(&mut node);
        self.write_node(page_num, &node, cols)?;
        result
    }

    pub fn cell_search(
        &self,
        page_num: usize,
        key: &Value,
        key_column: &Column,
    ) -> Result<(usize, bool), Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num)?;
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
                let mid_key = Node::read_key_at(&page.data, mid, key_column, self.row_size)?;

                match mid_key.cmp(key) {
                    Ordering::Less => lo = mid + 1,
                    Ordering::Greater => hi = mid,
                    Ordering::Equal => return Ok((mid, true)),
                }
            }

            Ok((lo, false))
        }
    }

    pub fn leaf_search(
        &self,
        page_num: usize,
        key: &Value,
        key_column: &Column,
    ) -> Result<usize, Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num)?;
        if Node::read_is_leaf(&page.data) {
            Ok(page_num)
        } else {
            let m = Node::read_num_cells(&page.data) + 1;
            for i in 0..m {
                let node_key = Node::read_key_at(&page.data, i, key_column, self.row_size)?;

                if key.leq(&node_key) {
                    let child = Node::read_child_at(&page.data, i, key_column)?;
                    drop(pager);
                    return self.leaf_search(child, key, key_column);
                }
            }

            let child = Node::read_child_at(&page.data, m - 1, key_column)?;
            drop(pager);
            self.leaf_search(child, key, key_column)
        }
    }

    pub fn insert_into_leaf(
        &self,
        page_num: usize,
        key: &Value,
        row: &Row,
        cols: &[&Column],
    ) -> Result<(), Error> {
        let node = self.read_node(page_num, cols)?;
        let Node::Leaf { cells, .. } = node else {
            return Err(Error::WrongNodeType(
                "insert_into_leaf called on non-leaf".into(),
            ));
        };

        let (cell_num, is_new) = match cells.binary_search_by(|(k, _)| k.cmp(key)) {
            Ok(idx) => (idx, false), // duplicate key - overwrite
            Err(idx) => (idx, true), // new key - need to shift
        };

        if is_new {
            self.shift_cells_right(page_num, cell_num)?;
        }
        self.write_cell(page_num, cell_num, key, row, cols)?;
        Ok(())
    }

    pub fn insert_into_internal(
        &self,
        page_num: usize,
        split_key: &Value,
        new_child: usize,
        cols: &[&Column],
    ) -> Result<(), Error> {
        let mut node = self.read_node(page_num, cols)?;
        let Node::Internal {
            ref mut keys,
            ref mut children,
            ..
        } = node
        else {
            return Err(Error::WrongNodeType(
                "insert_into_internal called on non-internal".into(),
            ));
        };

        let idx = match keys.binary_search(split_key) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        };

        keys.insert(idx, split_key.clone());
        children.insert(idx + 1, new_child);

        self.write_node(page_num, &node, cols)?;
        Ok(())
    }

    pub fn split_leaf(&self, page_num: usize, cols: &[&Column]) -> Result<(Value, usize), Error> {
        let mut node = self.read_node(page_num, cols)?;
        let Node::Leaf {
            ref mut cells,
            ref mut next_leaf,
            parent,
        } = node
        else {
            return Err(Error::WrongNodeType("split_leaf called on non-leaf".into()));
        };

        let mid = cells.len() / 2;
        let right_cells: Vec<_> = cells.drain(mid..).collect();
        let split_key = right_cells[0].0.clone();

        let new_page = self.pager.borrow_mut().alloc_page();

        let right_node = Node::Leaf {
            parent,
            cells: right_cells,
            next_leaf: *next_leaf,
        };

        *next_leaf = Some(new_page);

        self.write_node(page_num, &node, cols)?;
        self.write_node(new_page, &right_node, cols)?;

        Ok((split_key, new_page))
    }

    pub fn split_internal(
        &self,
        page_num: usize,
        cols: &[&Column],
    ) -> Result<(Value, usize), Error> {
        let mut node = self.read_node(page_num, cols)?;
        let Node::Internal {
            ref mut keys,
            ref mut children,
            parent,
        } = node
        else {
            return Err(Error::WrongNodeType(
                "split_internal called on non-internal".into(),
            ));
        };

        let mid = keys.len() / 2;
        let split_key = keys[mid].clone();

        let right_keys: Vec<_> = keys.drain(mid + 1..).collect();
        let right_children: Vec<_> = children.drain(mid + 1..).collect();

        keys.pop();

        let new_page = self.pager.borrow_mut().alloc_page();

        let right_node = Node::Internal {
            parent,
            keys: right_keys,
            children: right_children.clone(),
        };

        self.write_node(page_num, &node, cols)?;
        self.write_node(new_page, &right_node, cols)?;

        for &child_page in &right_children {
            self.set_parent(child_page, new_page, cols)?;
        }

        Ok((split_key, new_page))
    }

    pub fn split_leaf_and_insert(
        &self,
        page_num: usize,
        key: &Value,
        row: &Row,
        cols: &[&Column],
    ) -> Result<(Value, usize), Error> {
        let (split_key, new_page) = self.split_leaf(page_num, cols)?;

        if key.leq(&split_key) {
            self.insert_into_leaf(page_num, key, row, cols)?;
        } else {
            self.insert_into_leaf(new_page, key, row, cols)?;
        }

        Ok((split_key, new_page))
    }

    pub fn split_internal_and_insert(
        &self,
        page_num: usize,
        key: &Value,
        new_child: usize,
        cols: &[&Column],
    ) -> Result<(Value, usize), Error> {
        let (split_key, new_page) = self.split_internal(page_num, cols)?;

        if key.leq(&split_key) {
            self.insert_into_internal(page_num, key, new_child, cols)?;
        } else {
            self.insert_into_internal(new_page, key, new_child, cols)?;
        }

        Ok((split_key, new_page))
    }

    pub fn insert_recursive(
        &self,
        page_num: usize,
        key: &Value,
        values: &Row,
        cols: &[&Column],
    ) -> Result<Option<(Value, usize)>, Error> {
        let node = self.read_node(page_num, cols)?;

        match node {
            Node::Internal { keys, children, .. } => {
                let child_idx = match keys.binary_search(key) {
                    Ok(idx) => idx + 1,
                    Err(idx) => idx,
                };

                if let Some((split_key, new_sibling)) =
                    self.insert_recursive(children[child_idx], key, values, cols)?
                {
                    if keys.len() < self.max_keys_per_internal {
                        self.insert_into_internal(page_num, &split_key, new_sibling, cols)?;
                        Ok(None)
                    } else {
                        Ok(Some(self.split_internal_and_insert(
                            page_num,
                            &split_key,
                            new_sibling,
                            cols,
                        )?))
                    }
                } else {
                    Ok(None)
                }
            }
            Node::Leaf { cells, .. } => {
                if cells.len() < self.max_cells_per_leaf {
                    self.insert_into_leaf(page_num, key, values, cols)?;
                    Ok(None)
                } else {
                    let (split_key, new_sibling) =
                        self.split_leaf_and_insert(page_num, key, values, cols)?;
                    Ok(Some((split_key, new_sibling)))
                }
            }
        }
    }

    pub fn insert(&self, key: &Value, row: &Row, cols: &[&Column]) -> Result<(), Error> {
        let old_root = self.root_page.get();
        if let Some((split_key, new_sibling)) = self.insert_recursive(old_root, key, row, cols)? {
            self.create_new_root(&split_key, old_root, new_sibling, cols)?;
        }
        Ok(())
    }

    pub fn create_new_root(
        &self,
        split_key: &Value,
        left_child: usize,
        right_child: usize,
        cols: &[&Column],
    ) -> Result<(), Error> {
        let new_root_page = self.pager.borrow_mut().alloc_page();

        let new_root = Node::Internal {
            parent: None,
            keys: vec![split_key.clone()],
            children: vec![left_child, right_child],
        };

        self.write_node(new_root_page, &new_root, cols)?;
        self.root_page.set(new_root_page);

        self.set_parent(left_child, new_root_page, cols)?;
        self.set_parent(right_child, new_root_page, cols)?;
        Ok(())
    }

    fn set_parent(
        &self,
        page_num: usize,
        parent_page: usize,
        cols: &[&Column],
    ) -> Result<(), Error> {
        let mut node = self.read_node(page_num, cols)?;
        match &mut node {
            Node::Internal { parent, .. } | Node::Leaf { parent, .. } => {
                *parent = Some(parent_page);
            }
        }
        self.write_node(page_num, &node, cols)?;
        Ok(())
    }

    pub fn shift_cells_right(&self, page_num: usize, from: usize) -> Result<(), Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num)?;

        let num_cells = Node::read_num_cells(&page.data);
        let shift_start = HEADER_SIZE + from * self.cell_size;
        let shift_end = HEADER_SIZE + num_cells * self.cell_size;

        page.data
            .copy_within(shift_start..shift_end, shift_start + self.cell_size);

        let new_count = (num_cells + 1) as u16;
        page.data[6..8].copy_from_slice(&new_count.to_le_bytes());
        Ok(())
    }

    pub fn write_cell(
        &self,
        page_num: usize,
        cell_num: usize,
        key: &Value,
        row: &Row,
        cols: &[&Column],
    ) -> Result<(), Error> {
        let mut pager = self.pager.borrow_mut();
        let page = pager.get_page(page_num)?;

        let offset = HEADER_SIZE + cell_num * self.cell_size;
        key.serialize(&mut page.data[offset..], self.key_size);

        row.serialize(cols.to_vec(), &mut page.data[offset + self.key_size..]);
        Ok(())
    }
}
