pub const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
pub struct Page {
    pub data: [u8; PAGE_SIZE],
}

#[derive(Debug)]
pub struct Pager {
    pages: Vec<Option<Page>>,
}

impl Pager {
    pub fn new() -> Self {
        Pager { pages: vec![] }
    }

    pub fn get_page(&mut self, page_num: usize) -> &mut Page {
        self.pages[page_num].as_mut().unwrap()
    }
}

impl Default for Pager {
    fn default() -> Self {
        Self::new()
    }
}
