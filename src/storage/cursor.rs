use super::node::Node;
use super::table::Table;
use crate::error::Error;
use crate::types::Value;

#[derive(Debug)]
pub struct Cursor<'a> {
    pub table: &'a Table,
    pub eot: bool,
    pub page_num: usize,
    pub cell_num: usize,
}

impl Cursor<'_> {
    pub fn advance(&mut self) -> Result<(), Error> {
        self.cell_num += 1;
        self.refresh()
    }

    pub fn refresh(&mut self) -> Result<(), Error> {
        let node = self.read_node()?;
        if self.cell_num >= node.num_cells() {
            match node.next_leaf() {
                Some(next_page) => {
                    self.page_num = next_page;
                    self.cell_num = 0;
                }
                None => {
                    self.eot = true;
                }
            }
        }
        Ok(())
    }

    pub fn read_node(&self) -> Result<Node, Error> {
        self.table.read_node(self.page_num)
    }

    pub fn write_node(&self, node: &Node) -> Result<(), Error> {
        self.table.write_node(self.page_num, node)
    }

    pub fn shift_cells_right(&self) -> Result<(), Error> {
        self.table.shift_cells_right(self.page_num, self.cell_num)
    }

    pub fn write_cell(&self, key: &Value, row: &[Value]) -> Result<(), Error> {
        self.table
            .write_cell(self.page_num, self.cell_num, key, row)
    }

    pub fn with_node<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: FnOnce(&Node) -> Result<R, Error>,
    {
        self.table.with_node(self.page_num, f)
    }

    pub fn with_node_mut<F, R>(&self, f: F) -> Result<R, Error>
    where
        F: FnOnce(&mut Node) -> Result<R, Error>,
    {
        self.table.with_node_mut(self.page_num, f)
    }
}
