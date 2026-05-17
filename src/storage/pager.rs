use crate::error::Error;
use std::cmp::max;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
pub const MAGIC: usize = 4096;
pub const PAGE_SIZE: usize = 4096;
pub const MAX_CACHE: usize = 32;

#[derive(Debug)]
pub struct Page {
    pub data: [u8; PAGE_SIZE],
}

#[derive(Debug)]
pub struct Pager {
    file: Option<File>,
    page_cache: HashMap<usize, Page>,
    queue: VecDeque<usize>,
    dirty: HashSet<usize>,
    next_page: usize,
}

impl Pager {
    pub fn new(file_path: &str) -> Result<Self, Error> {
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)?;

        let file_len = file.metadata()?.len() as usize;
        if file_len < MAGIC {
            file.set_len(MAGIC as u64)?;
        }

        let next_page = if file_len <= MAGIC {
            0
        } else {
            (file_len - MAGIC) / PAGE_SIZE
        };

        Ok(Pager {
            file: Some(file),
            page_cache: HashMap::new(),
            queue: VecDeque::new(),
            dirty: HashSet::new(),
            next_page,
        })
    }

    pub fn memory() -> Self {
        Pager {
            file: None,
            page_cache: HashMap::new(),
            queue: VecDeque::new(),
            dirty: HashSet::new(),
            next_page: 0,
        }
    }

    pub fn get_page(&mut self, page_num: usize) -> Result<&mut Page, Error> {
        if !self.page_cache.contains_key(&page_num) {
            // Eviction (only if file-backed)
            if let Some(ref mut file) = self.file {
                if self.page_cache.len() >= MAX_CACHE {
                    let to_evict = self.queue.pop_back().unwrap();

                    // Write dirty page to disk before evicting
                    if self.dirty.contains(&to_evict) {
                        let page = self.page_cache.get(&to_evict).unwrap();
                        let offset = MAGIC + (PAGE_SIZE * to_evict);
                        file.seek(SeekFrom::Start(offset as u64))?;
                        file.write_all(&page.data)?;
                        self.dirty.remove(&to_evict);
                    }
                    self.page_cache.remove(&to_evict);
                }

                let mut buf = [0; PAGE_SIZE];
                let offset = MAGIC + (PAGE_SIZE * page_num);
                file.seek(SeekFrom::Start(offset as u64))?;
                let _ = file.read(&mut buf)?;
                self.queue.push_front(page_num);
                self.page_cache.insert(page_num, Page { data: buf });
            } else {
                //  just create empty page if we're just workin in memory
                self.page_cache.insert(
                    page_num,
                    Page {
                        data: [0; PAGE_SIZE],
                    },
                );
            }
        }

        self.dirty.insert(page_num);
        Ok(self.page_cache.get_mut(&page_num).unwrap())
    }

    pub fn alloc_page(&mut self) -> usize {
        let page_num = self.next_page;
        self.next_page += 1;
        page_num
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        if let Some(ref mut file) = self.file {
            for &page_num in &self.dirty {
                if let Some(page) = self.page_cache.get(&page_num) {
                    let offset = MAGIC + (PAGE_SIZE * page_num);
                    file.seek(SeekFrom::Start(offset as u64))
                        .expect("Failed to seek to page");
                    file.write_all(&page.data).expect("failed to write to page");
                }
            }
        }
    }
}
