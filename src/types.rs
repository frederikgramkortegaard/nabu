use crate::sql::ast::Value;
use crate::storage::Table;
use crate::tree::Node;

#[derive(Debug)]
pub enum QueryResult {
    Insert { rows_affected: u64 },
    Select { rows: Vec<Vec<Value>> },
}
#[derive(Debug)]
pub struct Cursor<'a> {
    pub table: &'a Table,
    pub eot: bool,
    pub page_num: usize,
    pub cell_num: usize,
}

impl Cursor<'_> {
    pub fn advance(&mut self) {
        self.cell_num += 1;

        let node = self.read_node();
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
    }

    pub fn read_node(&self) -> Node {
        self.table.read_node(self.page_num)
    }

    pub fn write_node(&self, node: &Node) {
        self.table.write_node(self.page_num, node)
    }

    pub fn shift_cells_right(&self) {
        self.table.shift_cells_right(self.page_num, self.cell_num)
    }

    pub fn write_cell(&self, key: &Value, row: &[Value]) {
        self.table
            .write_cell(self.page_num, self.cell_num, key, row)
    }
}
