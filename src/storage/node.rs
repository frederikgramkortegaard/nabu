use crate::error::Error;
use crate::types::{Column, Row, Value};

pub const HEADER_SIZE: usize = 12;

/*
 * NODE HEADER (12 bytes)
 * ----------------------
 * +0   u8    is_leaf      0=internal, !0=leaf
 * +1   u8    is_root      0=root, 1=not root
 * +2   u32   parent       0xFFFFFFFF=NULL
 * +6   u16   num_cells
 * +8   u32   right_child  (internal) \_ same field,
 *            next_leaf    (leaf)     /  0xFFFFFFFF=NULL
 *
 * INTERNAL NODE (is_leaf=0)
 * -------------------------
 * [HEADER][child0|key0][child1|key1]...[childN-1|keyN-1]
 *          |_4B_||_ks_|
 *
 * right_child in header = childN (rightmost)
 * N keys => N+1 children
 *
 * LEAF NODE (is_leaf=1)
 * ---------------------
 * [HEADER][key0|row0][key1|row1]...[keyN-1|rowN-1]
 *          |ks||_rs_|
 *
 * next_leaf => sibling pointer for range scans
 *
 * ks = key_size, rs = row_size
 */

#[derive(Debug)]
pub enum Node {
    Internal {
        parent: Option<usize>,
        keys: Vec<Value>,
        children: Vec<usize>,
    },
    Leaf {
        parent: Option<usize>,
        cells: Vec<(Value, Row)>,
        next_leaf: Option<usize>,
    },
}

impl Node {
    pub fn is_root(&self) -> bool {
        match self {
            Node::Internal { parent, .. } | Node::Leaf { parent, .. } => parent.is_none(),
        }
    }

    pub fn read_is_root(src: &[u8]) -> bool {
        src[1] == 0
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
    pub fn read_next_leaf(src: &[u8]) -> Option<usize> {
        let ptr_bytes: [u8; 4] = src[8..12].try_into().unwrap();
        let ptr_raw = u32::from_le_bytes(ptr_bytes);
        if Node::read_is_leaf(src) && ptr_raw != 0xffffffff {
            Some(ptr_raw as usize)
        } else {
            None
        }
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self, Node::Leaf { .. })
    }
    pub fn read_is_leaf(src: &[u8]) -> bool {
        src[0] != 0
    }

    pub fn read_right_child(src: &[u8]) -> usize {
        let bytes: [u8; 4] = src[8..12].try_into().unwrap();
        u32::from_le_bytes(bytes) as usize
    }

    pub fn read_left_child(src: &[u8]) -> usize {
        let bytes: [u8; 4] = src[12..16].try_into().unwrap();
        u32::from_le_bytes(bytes) as usize
    }

    pub fn get_parent(&self) -> Option<usize> {
        match self {
            Node::Internal { parent, .. } | Node::Leaf { parent, .. } => *parent,
        }
    }

    pub fn read_parent(src: &[u8]) -> Option<usize> {
        let parent_bytes: [u8; 4] = src[2..6].try_into().unwrap();
        let parent_raw = u32::from_le_bytes(parent_bytes);
        if parent_raw == 0xffffffff {
            None
        } else {
            Some(parent_raw as usize)
        }
    }

    pub fn read_child_at(src: &[u8], n: usize, key_column: &Column) -> Result<usize, Error> {
        if Node::read_is_leaf(src) {
            return Err(Error::WrongNodeType(
                "read_child_at can only be called on data that belongs to an internal node".into(),
            ));
        }

        let num_cells = Self::read_num_cells(src);
        if n > num_cells {
            // +1 because there is also a child in the header
            return Err(Error::OutOfBounds {
                index: n,
                len: num_cells,
            });
        }

        // The right-most child is stored at src[8..12] for easy traversal, because of that
        // we need to handle that case specially
        let smart_src: [u8; 4] = if n == num_cells {
            src[8..12].try_into().unwrap()
        } else {
            let offset = HEADER_SIZE + n * (4 + key_column.column_size);
            src[offset..offset + 4].try_into().unwrap()
        };

        let ptr_bytes: [u8; 4] = smart_src;
        let ptr_raw = u32::from_le_bytes(ptr_bytes);
        if ptr_raw == 0xffffffff {
            Err(Error::CorruptedTree(
                "Internal nodes should never have a NULL child pointer".into(),
            ))
        } else {
            Ok(ptr_raw as usize)
        }
    }

    pub fn read_row_at(
        src: &[u8],
        n: usize,
        key_column: &Column,
        columns: Vec<&Column>,
        row_size: usize,
    ) -> Result<Row, Error> {
        if !Node::read_is_leaf(src) {
            return Err(Error::WrongNodeType(
                "read_row_at can only be called on leaf nodes".into(),
            ));
        }

        let num_cells = Self::read_num_cells(src);
        if n >= num_cells {
            return Err(Error::OutOfBounds {
                index: n,
                len: num_cells,
            });
        }

        let offset = HEADER_SIZE + n * (key_column.column_size + row_size) + key_column.column_size;
        Ok(Row::deserialize(&columns, &src[offset..offset + row_size]))
    }

    pub fn read_key_at(
        src: &[u8],
        n: usize,
        key_column: &Column,
        row_size: usize,
    ) -> Result<Value, Error> {
        let num_cells = Self::read_num_cells(src);
        if n >= num_cells {
            return Err(Error::OutOfBounds {
                index: n,
                len: num_cells,
            });
        }

        let offset = if Node::read_is_leaf(src) {
            HEADER_SIZE + n * (key_column.column_size + row_size)
        } else {
            HEADER_SIZE + n * (4 + key_column.column_size) + 4
        };

        Ok(Value::deserialize(&src[offset..], key_column))
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
            Node::Internal { keys, children, .. } => {
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
                cells, next_leaf, ..
            } => {
                match next_leaf {
                    Some(nl) => dest[8..12].copy_from_slice(&(*nl as u32).to_le_bytes()),
                    None => dest[8..12].fill(0xff),
                }

                for (key, row) in cells {
                    key.serialize(&mut dest[offset..offset + key_size], key_size);
                    offset += key_size;
                    row.serialize(columns.to_vec(), &mut dest[offset..]);
                    offset += row_size;
                }
            }
        }
    }

    pub fn deserialize(src: &[u8], key_size: usize, row_size: usize, columns: &[&Column]) -> Node {
        let is_leaf = src[0] != 0;

        let parent = Self::read_parent(src);
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

            for _ in 0..num_cells {
                let key = Value::deserialize(&src[offset..], &columns[0]);
                offset += key_size;

                let row = Row::deserialize(&columns.to_vec(), &src[offset..offset + row_size]);
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
                let child_bytes: [u8; 4] = src[offset..offset + 4].try_into().unwrap();
                children.push(u32::from_le_bytes(child_bytes) as usize);
                offset += 4;

                let key = Value::deserialize(&src[offset..], &columns[0]);
                keys.push(key);
                offset += key_size;
            }

            children.push(right_child);

            Node::Internal {
                parent,
                keys,
                children,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ColumnType;
    use ordered_float::OrderedFloat;

    fn num(n: f64) -> Value {
        Value::Number(OrderedFloat(n))
    }

    fn make_key_column() -> Column {
        Column::new("id".into(), ColumnType::Number)
    }

    // Manually build a leaf node header
    fn make_leaf_header(parent: Option<u32>, num_cells: u16, next_leaf: Option<u32>) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0] = 1; // is_leaf = true
        buf[1] = if parent.is_none() { 0 } else { 1 }; // is_root
        match parent {
            Some(p) => buf[2..6].copy_from_slice(&p.to_le_bytes()),
            None => buf[2..6].fill(0xff),
        }
        buf[6..8].copy_from_slice(&num_cells.to_le_bytes());
        match next_leaf {
            Some(nl) => buf[8..12].copy_from_slice(&nl.to_le_bytes()),
            None => buf[8..12].fill(0xff),
        }
        buf
    }

    // Manually build an internal node header
    fn make_internal_header(parent: Option<u32>, num_cells: u16, right_child: u32) -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0] = 0; // is_leaf = false
        buf[1] = if parent.is_none() { 0 } else { 1 };
        match parent {
            Some(p) => buf[2..6].copy_from_slice(&p.to_le_bytes()),
            None => buf[2..6].fill(0xff),
        }
        buf[6..8].copy_from_slice(&num_cells.to_le_bytes());
        buf[8..12].copy_from_slice(&right_child.to_le_bytes());
        buf
    }

    #[test]
    fn test_read_is_leaf() {
        let leaf = make_leaf_header(None, 0, None);
        let internal = make_internal_header(None, 0, 0);

        assert!(Node::read_is_leaf(&leaf));
        assert!(!Node::read_is_leaf(&internal));
    }

    #[test]
    fn test_read_is_root() {
        let root = make_leaf_header(None, 0, None);
        let non_root = make_leaf_header(Some(5), 0, None);

        assert!(Node::read_is_root(&root));
        assert!(!Node::read_is_root(&non_root));
    }

    #[test]
    fn test_read_parent() {
        let root = make_leaf_header(None, 0, None);
        let child = make_leaf_header(Some(42), 0, None);

        assert_eq!(Node::read_parent(&root), None);
        assert_eq!(Node::read_parent(&child), Some(42));
    }

    #[test]
    fn test_read_num_cells() {
        let header = make_leaf_header(None, 7, None);
        assert_eq!(Node::read_num_cells(&header), 7);

        let header2 = make_internal_header(None, 123, 0);
        assert_eq!(Node::read_num_cells(&header2), 123);
    }

    #[test]
    fn test_read_next_leaf() {
        let with_next = make_leaf_header(None, 0, Some(99));
        let without_next = make_leaf_header(None, 0, None);
        let internal = make_internal_header(None, 0, 99);

        assert_eq!(Node::read_next_leaf(&with_next), Some(99));
        assert_eq!(Node::read_next_leaf(&without_next), None);
        assert_eq!(Node::read_next_leaf(&internal), None); // internal nodes don't have next_leaf
    }

    #[test]
    fn test_read_right_child() {
        let header = make_internal_header(None, 0, 42);
        assert_eq!(Node::read_right_child(&header), 42);
    }

    #[test]
    fn test_serialize_deserialize_leaf_roundtrip() {
        let key_col = make_key_column();
        let cols: Vec<&Column> = vec![&key_col];
        let key_size = key_col.column_size;
        let row_size = key_size;

        let node = Node::Leaf {
            parent: None,
            cells: vec![
                (num(1.0), Row(vec![num(1.0)])),
                (num(2.0), Row(vec![num(2.0)])),
                (num(3.0), Row(vec![num(3.0)])),
            ],
            next_leaf: Some(5),
        };

        let mut buf = vec![0u8; 1024];
        node.serialize(&mut buf, key_size, row_size, &cols);

        let deserialized = Node::deserialize(&buf, key_size, row_size, &cols);

        assert!(matches!(deserialized, Node::Leaf { .. }));
        if let Node::Leaf {
            parent,
            cells,
            next_leaf,
        } = deserialized
        {
            assert_eq!(parent, None);
            assert_eq!(cells.len(), 3);
            assert_eq!(next_leaf, Some(5));
            assert_eq!(cells[0].0, num(1.0));
            assert_eq!(cells[1].0, num(2.0));
            assert_eq!(cells[2].0, num(3.0));
        }
    }

    #[test]
    fn test_serialize_deserialize_internal_roundtrip() {
        let key_col = make_key_column();
        let cols: Vec<&Column> = vec![&key_col];
        let key_size = key_col.column_size;
        let row_size = key_size;

        let node = Node::Internal {
            parent: Some(0),
            keys: vec![num(10.0), num(20.0)],
            children: vec![1, 2, 3],
        };

        let mut buf = vec![0u8; 1024];
        node.serialize(&mut buf, key_size, row_size, &cols);

        let deserialized = Node::deserialize(&buf, key_size, row_size, &cols);

        assert!(matches!(deserialized, Node::Internal { .. }));
        if let Node::Internal {
            parent,
            keys,
            children,
        } = deserialized
        {
            assert_eq!(parent, Some(0));
            assert_eq!(keys.len(), 2);
            assert_eq!(children.len(), 3);
            assert_eq!(keys[0], num(10.0));
            assert_eq!(keys[1], num(20.0));
            assert_eq!(children, vec![1, 2, 3]);
        }
    }

    #[test]
    fn test_read_key_at_leaf() {
        let key_col = make_key_column();
        let cols: Vec<&Column> = vec![&key_col];
        let key_size = key_col.column_size;
        let row_size = key_size;

        let node = Node::Leaf {
            parent: None,
            cells: vec![
                (num(100.0), Row(vec![num(100.0)])),
                (num(200.0), Row(vec![num(200.0)])),
            ],
            next_leaf: None,
        };

        let mut buf = vec![0u8; 1024];
        node.serialize(&mut buf, key_size, row_size, &cols);

        assert_eq!(
            Node::read_key_at(&buf, 0, &key_col, row_size).unwrap(),
            num(100.0)
        );
        assert_eq!(
            Node::read_key_at(&buf, 1, &key_col, row_size).unwrap(),
            num(200.0)
        );
        assert!(Node::read_key_at(&buf, 2, &key_col, row_size).is_err());
    }

    #[test]
    fn test_read_key_at_internal() {
        let key_col = make_key_column();
        let cols: Vec<&Column> = vec![&key_col];
        let key_size = key_col.column_size;
        let row_size = key_size;

        let node = Node::Internal {
            parent: None,
            keys: vec![num(50.0), num(100.0)],
            children: vec![0, 1, 2],
        };

        let mut buf = vec![0u8; 1024];
        node.serialize(&mut buf, key_size, row_size, &cols);

        assert_eq!(
            Node::read_key_at(&buf, 0, &key_col, row_size).unwrap(),
            num(50.0)
        );
        assert_eq!(
            Node::read_key_at(&buf, 1, &key_col, row_size).unwrap(),
            num(100.0)
        );
        assert!(Node::read_key_at(&buf, 2, &key_col, row_size).is_err());
    }

    #[test]
    fn test_read_child_at() {
        let key_col = make_key_column();
        let cols: Vec<&Column> = vec![&key_col];
        let key_size = key_col.column_size;
        let row_size = key_size;

        let node = Node::Internal {
            parent: None,
            keys: vec![num(50.0), num(100.0)],
            children: vec![10, 20, 30], // 3 children for 2 keys
        };

        let mut buf = vec![0u8; 1024];
        node.serialize(&mut buf, key_size, row_size, &cols);

        assert_eq!(Node::read_child_at(&buf, 0, &key_col).unwrap(), 10);
        assert_eq!(Node::read_child_at(&buf, 1, &key_col).unwrap(), 20);
        assert_eq!(Node::read_child_at(&buf, 2, &key_col).unwrap(), 30); // rightmost
        assert!(Node::read_child_at(&buf, 3, &key_col).is_err()); // out of bounds
    }

    #[test]
    fn test_read_child_at_leaf_errors() {
        let key_col = make_key_column();
        let cols: Vec<&Column> = vec![&key_col];
        let key_size = key_col.column_size;
        let row_size = key_size;

        let node = Node::Leaf {
            parent: None,
            cells: vec![(num(1.0), Row(vec![num(1.0)]))],
            next_leaf: None,
        };

        let mut buf = vec![0u8; 1024];
        node.serialize(&mut buf, key_size, row_size, &cols);

        // Should error because it's a leaf, not internal
        assert!(Node::read_child_at(&buf, 0, &key_col).is_err());
    }
}
