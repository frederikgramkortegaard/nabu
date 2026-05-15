use crate::sql::ast::Value;
use crate::storage::Table;

#[derive(Debug)]
pub enum QueryResult {
    Insert { rows_affected: u64 },
    Select { rows: Vec<Vec<Value>> },
}
#[derive(Debug)]
pub struct Cursor<'a> {
    pub table: &'a Table,
    pub row: usize,
    pub eot: bool,
}

impl Cursor<'_> {
    pub fn advance(&mut self) {
        self.row += 1;
        self.eot = self.row >= self.table.rows.get();
    }

    pub fn with_row<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut pager = self.table.pager.borrow_mut();
        let page = pager.get_page(self.row / self.table.rows_per_page);
        let row_offset = (self.row % self.table.rows_per_page) * self.table.row_size;
        f(&mut page.data[row_offset..row_offset + self.table.row_size])
    }
}
