use crate::sql::ast::Value;
use crate::storage::table::*;

pub const HEADER_SIZE: usize = 12;

#[derive(Debug)]
pub enum Node {
    Internal {
        parent: Option<usize>,
        keys: Vec<Value>,
        children: Vec<usize>,
    },
    Leaf {
        parent: Option<usize>,
        cells: Vec<(Value, Vec<Value>)>,
        next_leaf: Option<usize>,
    },
}

impl Node {
    pub fn is_root(&self) -> bool {
        match self {
            Node::Internal { parent, .. } | Node::Leaf { parent, .. } => parent.is_none(),
        }
    }
    pub fn num_cells(&self) -> usize {
        match self {
            Node::Internal { keys, .. } => keys.len(),
            Node::Leaf { cells, .. } => cells.len(),
        }
    }
    pub fn read_num_cells(src: &[u8]) -> usize {
        let bytes: [u8; 2] = src[6..8].try_into().unwrap();
        u16::from_le_bytes(bytes) as usize
    }

    pub fn next_leaf(&self) -> Option<usize> {
        match self {
            Node::Leaf { next_leaf, .. } => *next_leaf,
            Node::Internal { .. } => None,
        }
    }

    pub fn is_leaf(src: &[u8]) -> bool {
        src[0] != 0
    }

    pub fn read_right_child(src: &[u8]) -> usize {
        let bytes: [u8; 4] = src[8..12].try_into().unwrap();
        u32::from_le_bytes(bytes) as usize
    }

    pub fn read_left_child(src: &[u8]) -> usize {
        // First cell starts at HEADER_SIZE (12), child pointer is first 4 bytes
        let bytes: [u8; 4] = src[12..16].try_into().unwrap();
        u32::from_le_bytes(bytes) as usize
    }

    pub fn get_parent(&self) -> Option<usize> {
        match self {
            Node::Internal { parent, .. } | Node::Leaf { parent, .. } => *parent,
        }
    }
    pub fn serialize(
        &self,
        dest: &mut [u8],
        key_size: usize,
        row_size: usize,
        columns: &[&Column],
    ) {
        dest[0] = u8::from(!matches!(self, Node::Internal { .. }));
        dest[1] = u8::from(!self.is_root());

        let parent = self.get_parent();

        match parent {
            Some(p) => dest[2..6].copy_from_slice(&(p as u32).to_le_bytes()),
            None => dest[2..6].fill(0xff),
        }
        dest[6..8].copy_from_slice(&(self.num_cells() as u16).to_le_bytes());

        let mut offset = 12;
        match self {
            Node::Internal {
                parent,
                keys,
                children,
            } => {
                match children.last() {
                    Some(c) => dest[8..12].copy_from_slice(&(*c as u32).to_le_bytes()),
                    None => dest[8..12].fill(0xff),
                }

                for (key, child) in std::iter::zip(keys, children) {
                    dest[offset..offset + 4].copy_from_slice(&(*child as u32).to_le_bytes());
                    key.serialize(&mut dest[offset + 4..offset + 4 + key_size], key_size);
                    offset += 4 + key_size;
                }
            }
            Node::Leaf {
                parent,
                cells,
                next_leaf,
            } => {
                match next_leaf {
                    Some(nl) => dest[8..12].copy_from_slice(&(*nl as u32).to_le_bytes()),
                    None => dest[8..12].fill(0xff),
                }

                for (key, values) in cells {
                    key.serialize(&mut dest[offset..offset + key_size], key_size);
                    offset += key_size;
                    serialize_row(values, columns.to_vec(), &mut dest[offset..]);
                    offset += row_size;
                }
            }
        }
    }

    pub fn deserialize(src: &[u8], key_size: usize, row_size: usize, columns: &[&Column]) -> Node {
        let is_leaf = src[0] != 0;
        let _is_root = src[1] == 0;

        let parent_bytes: [u8; 4] = src[2..6].try_into().unwrap();
        let parent_raw = u32::from_le_bytes(parent_bytes);
        let parent = if parent_raw == 0xffffffff {
            None
        } else {
            Some(parent_raw as usize)
        };

        let num_cells = Self::read_num_cells(src);

        let ptr_bytes: [u8; 4] = src[8..12].try_into().unwrap();
        let ptr_raw = u32::from_le_bytes(ptr_bytes);

        if is_leaf {
            let next_leaf = if ptr_raw == 0xffffffff {
                None
            } else {
                Some(ptr_raw as usize)
            };

            let mut cells = Vec::with_capacity(num_cells);
            let mut offset = HEADER_SIZE;
            let cell_size = key_size + row_size;

            for _ in 0..num_cells {
                // Deserialize key (first column)
                let key_col = &columns[0];
                let key =
                    deserialize_row(&vec![key_col], &src[offset..offset + key_size])[0].clone();
                offset += key_size;

                // Deserialize row
                let row = deserialize_row(&columns.to_vec(), &src[offset..offset + row_size]);
                offset += row_size;

                cells.push((key, row));
            }

            Node::Leaf {
                parent,
                cells,
                next_leaf,
            }
        } else {
            let right_child = ptr_raw as usize;

            let mut keys = Vec::with_capacity(num_cells);
            let mut children = Vec::with_capacity(num_cells + 1);
            let mut offset = HEADER_SIZE;

            for _ in 0..num_cells {
                // Child pointer
                let child_bytes: [u8; 4] = src[offset..offset + 4].try_into().unwrap();
                children.push(u32::from_le_bytes(child_bytes) as usize);
                offset += 4;

                // Key
                let key_col = &columns[0];
                let key =
                    deserialize_row(&vec![key_col], &src[offset..offset + key_size])[0].clone();
                keys.push(key);
                offset += key_size;
            }

            // Add right child
            children.push(right_child);

            Node::Internal {
                parent,
                keys,
                children,
            }
        }
    }
}
