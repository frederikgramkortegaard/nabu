use crate::error::Error;
use crate::storage::pager::PAGE_SIZE;
pub const MAGIC_BYTES: &[u8; 4] = b"RSDB";
pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug)]
pub struct DatabaseHeader {
    pub version: u32,
    pub page_size: usize,
    pub system_table_page: usize,
}

/*

=================
|    HEADER     |
=================
MAGIC : [0..4]
VERSION : [4..8]
SYSTEM TABLE ROOT : [8..12]
*/

impl DatabaseHeader {
    pub fn new(system_table_page: usize) -> Self {
        Self {
            version: CURRENT_VERSION,
            page_size: PAGE_SIZE,
            system_table_page,
        }
    }

    pub fn serialize(&self, root_page: usize, dest: &mut [u8]) -> Result<(), Error> {
        dest[0..4].copy_from_slice(MAGIC_BYTES);
        dest[4..8].copy_from_slice(&(CURRENT_VERSION as u32).to_le_bytes());
        dest[8..12].copy_from_slice(&(root_page as u32).to_le_bytes());
        dest[12..16].copy_from_slice(&(PAGE_SIZE as u32).to_le_bytes());

        Ok(())
    }
    pub fn deserialize(src: &[u8]) -> Option<Self> {
        let magic_bytes: [u8; 4] = src[0..4].try_into().unwrap();

        if magic_bytes != *MAGIC_BYTES {
            return None;
        }

        let mut bytes: [u8; 4] = src[4..8].try_into().unwrap();
        let version: u32 = u32::from_le_bytes(bytes);

        bytes = src[8..12].try_into().unwrap();
        let system_table_page = u32::from_le_bytes(bytes) as usize;

        bytes = src[12..16].try_into().unwrap();
        let page_size = u32::from_le_bytes(bytes) as usize;

        Some(Self {
            version,
            page_size,
            system_table_page,
        })
    }
}
