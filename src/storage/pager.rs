pub const PAGE_SIZE: usize = 4096;

#[derive(Debug)]
pub struct Page {
    pub data: [u8; PAGE_SIZE],
}

#[derive(Debug)]
pub struct Pager {
    pages: Vec<Option<Box<Page>>>,
}

impl Pager {
    pub fn new() -> Self {
        Pager { pages: vec![] }
    }

    pub fn get_page(&mut self, page_num: usize) -> &mut Page {
        if page_num >= self.pages.len() {
            self.pages.resize_with(page_num + 1, || None);
        }

        if self.pages[page_num].is_none() {
            self.pages[page_num] = Some(Box::new(Page {
                data: [0; PAGE_SIZE],
            }));
        }

        self.pages[page_num].as_mut().unwrap()
    }
}

impl Default for Pager {
    fn default() -> Self {
        Self::new()
    }
}
