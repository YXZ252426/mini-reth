use std::collections::HashMap;
use rlp::{Rlp, RlpStream};

use crate::crypto::keccak256;
use crate::types::Hash;

pub type Nibble = u8;
pub type NodeRef = Hash;

fn pack_nibbles(nibbles: &[Nibble]) -> Vec<u8> {
    debug_assert_eq!(nibbles.len()%2, 0);
    debug_assert!(nibbles.iter().all(|nibble| *nibble < 16));

    nibbles
        .chunks_exact(2)
        .map(|pair| pair[0] << 4 | pair[1])
        .collect()
}

fn unpack_nibbles(bytes: &[u8]) -> Vec<Nibble> {
    let mut nibbles = Vec::with_capacity(bytes.len() * 2);

    for byte in bytes {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0f);
    }
    nibbles
}

pub fn compact_encode(path: &[Nibble], is_leaf: bool) -> Vec<u8> {
    assert!(
        path.iter().all(|nibble| *nibble < 16),
        "MPT path contains a non-nibble value"
    );

    let is_odd = path.len() % 2 == 1;
    let flag = match (is_leaf, is_odd) {
        (false, false) => 0,
        (false, true) => 1,
        (true, false) => 2,
        (true, true) => 3,
    };

    let mut encoded_nibbles = Vec::with_capacity(path.len() + 2);

    encoded_nibbles.push(flag);
    if !is_odd {
        encoded_nibbles.push(0);
    }
    encoded_nibbles.extend_from_slice(path);

    pack_nibbles(&encoded_nibbles)
}

pub fn compact_decode(encoded: &[u8]) -> Option<(Vec<Nibble>, bool)> {
    let nibbles = unpack_nibbles(encoded);
    let flag = *nibbles.first()?;
    let is_leaf = matches!(flag, 2 | 3);
    let is_odd = matches!(flag, 1 | 3);

    match (flag, is_odd) {
        (0..=3, true) => Some((nibbles[1..].to_vec(), is_leaf)),
        (0..=3, false) if nibbles.get(1) == Some(&0) => Some((nibbles[2..].to_vec(), is_leaf)),
        _ => None,
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MptNode {
    Branch {
        children: [Option<NodeRef>; 16],
        value: Option<Vec<u8>>,
    },
    Leaf {
        path: Vec<Nibble>,
        value: Vec<u8>,
    },
    Extension {
        path: Vec<Nibble>,
        child: NodeRef,
    },
}

impl MptNode {
    pub fn branch() -> Self {
        Self::Branch { 
            children: [None; 16],
            value: None,
        }
    }

    pub fn branch_with_value(value: Vec<u8>) -> Self {
        Self::Branch { 
            children: [None; 16], 
            value: Some(value) 
        }
    }

    pub fn leaf(path: Vec<Nibble>, value: Vec<u8>) -> Self {
        Self::Leaf { path, value }
    }

    pub fn extension(path: Vec<Nibble>, child: NodeRef) -> Self {
        Self::Extension { path, child}
    }

    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Branch { children, value } => {
                let mut stream = RlpStream::new_list(17);

                for child in children {
                    match child {
                        Some(hash) => {
                            let hash_bytes: &[u8] = hash;
                            stream.append(&hash_bytes);
                        }
                        None => {
                            stream.append_empty_data();
                        }
                    }
                }

                match value {
                    Some(value) => {
                        stream.append(value);
                    }
                    None => {
                        stream.append_empty_data();
                    }
                }

                stream.out().to_vec()
            },
            Self::Leaf { path, value } => {
                let mut stream = RlpStream::new_list(2);
                stream.append(&compact_encode(path, true));
                stream.append(value);
                stream.out().to_vec()
            },
            Self::Extension { path, child } => {
                let mut stream = RlpStream::new_list(2);
                stream.append(&compact_encode(path, false));

                let child_bytes: &[u8] = child;
                stream.append(&child_bytes);

                stream.out().to_vec()
            },
        }
    }

    pub fn decode(encoded: &[u8]) -> Option<Self> {
        let rlp = Rlp::new(encoded);
        let item_count = rlp.item_count().ok()?;

        match item_count {
            2 => Self::decode_short_node(&rlp),
            17 => Self::decode_branch(&rlp),
            _ => None
        }
    }

    fn decode_branch(rlp: &Rlp<'_>) -> Option<Self> {
        let mut children = [None; 16];
        
        for (index, child) in children.iter_mut().enumerate() {
            let data: Vec<u8> = rlp.val_at(index).ok()?;

            if data.is_empty() {
                continue;
            }

            if data.len() != 32 {
                return None;
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data);
            *child = Some(hash);
        }

        let value: Vec<u8> = rlp.val_at(16).ok()?;
        let value  = if value.is_empty() {None} else {Some(value)};

        Some(Self::Branch { children, value })
    }

    fn decode_short_node(rlp: &Rlp<'_>) -> Option<Self> {
        let encoded_path: Vec<u8> = rlp.val_at(0).ok()?;
        let (path, is_leaf) = compact_decode(&encoded_path)?;

        if is_leaf {
            let value: Vec<u8> = rlp.val_at(1).ok()?;
            return Some(Self::Leaf { path, value })
        }

        let child_vec: Vec<u8> = rlp.val_at(1).ok()?;
        if child_vec.len() != 32 {
            return None;
        }

        let mut child = [0u8; 32];
        child.copy_from_slice(&child_vec);

        Some(Self::Extension { path, child })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MptNodeDb {
    nodes: HashMap<Hash, Vec<u8>>,
}

impl MptNodeDb {
    pub fn new() -> Self {
        Self { nodes: HashMap::new(), }
    }

    pub fn put(&mut self, node: &MptNode) -> Hash {
        let encoded = node.encode();
        let hash = keccak256(&encoded);

        self.nodes.insert(hash, encoded);
        
        hash
    }

    pub fn get(&self, hash: Hash) -> Option<MptNode> {
        let encoded = self.nodes.get(&hash)?;

        MptNode::decode(encoded)
    }

    pub fn get_encoded(&self, hash: Hash) -> Option<&[u8]> {
        self.nodes.get(&hash).map(Vec::as_slice)
    }

    pub fn contains(&self, hash: Hash) -> bool {
        self.nodes.contains_key(&hash)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_starts_empty() {
        let node = MptNode::branch();

        let MptNode::Branch { children, value } = node else {
            panic!("expected branch node");
        };

        assert!(children.iter().all(Option::is_none));
        assert_eq!(value, None);
    }

    #[test]
    fn branch_can_store_value_for_key_that_ends_at_bench() {
        let node = MptNode::branch_with_value(b"account".to_vec());

        let MptNode::Branch { children, value } = node else {
            panic!("expected branch node");
        };

        assert!(children.iter().all(Option::is_none));
        assert_eq!(value, Some(b"account".to_vec()));
    }

    #[test]
    fn leaf_stores_remaining_path_and_value() {
        let node = MptNode::leaf(vec![0x0a, 0x0b, 0x0c], b"value".to_vec());

        assert_eq!(
            node,
            MptNode::Leaf {
                path: vec![0x0a, 0x0b, 0x0c],
                value: b"value".to_vec(),
            }
        );
    }

    #[test]
    fn extension_stores_shared_path_and_child_reference() {
        let child = [0x11u8; 32];
        let node = MptNode::extension(vec![0x01, 0x02], child);

        assert_eq!(
            node,
            MptNode::Extension {
                path: vec![0x01, 0x02],
                child,
            }
        );
    }

    #[test]
    fn compact_encode_extension_even_path() {
        assert_eq!(
            compact_encode(&[0x01, 0x02, 0x03, 0x04], false),
            vec![0x00, 0x12, 0x34]
        );
    }

    #[test]
    fn compact_encode_extension_odd_path() {
        assert_eq!(
            compact_encode(&[0x01, 0x02, 0x03], false),
            vec![0x11, 0x23]
        );       
    }

    #[test]
    fn compact_encode_leaf_even_path() {
        assert_eq!(
            compact_encode(&[0x01, 0x02, 0x03, 0x04], true),
            vec![0x20, 0x12, 0x34]
        );
    }

    #[test]
    fn compact_encode_leaf_odd_path() {
        assert_eq!(compact_encode(&[0x01, 0x02, 0x03], true), vec![0x31, 0x23]);
    }

    #[test]
    fn compact_encode_empty_paths() {
        assert_eq!(compact_encode(&[], false), vec![0x00]);
        assert_eq!(compact_encode(&[], true), vec![0x20]);
    }

    #[test]
    fn compact_decode_extension_even_path() {
        assert_eq!(
            compact_decode(&[0x00, 0x12, 0x34]),
            Some((vec![0x01, 0x02, 0x03, 0x04], false))
        );
    }

    #[test]
    fn compact_decode_extension_odd_path() {
        assert_eq!(
            compact_decode(&[0x11, 0x23]),
            Some((vec![0x01, 0x02, 0x03], false))
        );
    }

    #[test]
    fn compact_decode_leaf_even_path() {
        assert_eq!(
            compact_decode(&[0x20, 0x12, 0x34]),
            Some((vec![0x01, 0x02, 0x03, 0x04], true))
        );
    }

    #[test]
    fn compact_decode_leaf_odd_path() {
        assert_eq!(
            compact_decode(&[0x31, 0x23]),
            Some((vec![0x01, 0x02, 0x03], true))
        );
    }

    #[test]
    fn compact_decode_rejects_invalid_inputs() {
        assert_eq!(compact_decode(&[]), None);
        assert_eq!(compact_decode(&[0x40]), None);
        assert_eq!(compact_decode(&[0x01]), None);
    }

    #[test]
    fn compact_encoding_round_trips_paths() {
        let paths = [vec![], vec![0x0f], vec![0x00, 0x01], vec![0x0a, 0x0b, 0x0c]];

        for path in paths {
            assert_eq!(
                compact_decode(&compact_encode(&path, false)),
                Some((path.clone(), false))
            );
            assert_eq!(
                compact_decode(&compact_encode(&path, true)),
                Some((path.clone(), true))
            );
        }
    }

    #[test]
    fn branch_node_rlp_round_trips() {
        let mut children = [None; 16];
        children[3] = Some([0x33u8; 32]);
        children[10] = Some([0xaau8; 32]);
        let node = MptNode::Branch { 
            children,
            value: Some(b"branch-value".to_vec()), 
        };

        assert_eq!(MptNode::decode(&node.encode()) , Some(node))
    }

    #[test]
    fn leaf_node_rlp_round_trips() {
        let node = MptNode::Leaf { 
            path: vec![0x01, 0x02, 0x03],
            value: b"leaf-node".to_vec() 
        };

        assert_eq!(MptNode::decode(&node.encode()), Some(node));
    }

    #[test]
    fn extension_node_rlp_round_trips() {
        let node = MptNode::Extension { 
            path: vec![0x01, 0x02, 0x03],
            child: [0x44u8; 32],
        };

        assert_eq!(MptNode::decode(&node.encode()), Some(node));
    }


    #[test]
    fn empty_branch_node_rlp_round_trips() {
        let node = MptNode::branch();

        assert_eq!(MptNode::decode(&node.encode()), Some(node));
    }

   #[test]
    fn decode_rejects_branch_child_with_invalid_hash_length() {
        let mut stream = RlpStream::new_list(17);

        stream.append(&vec![0x01, 0x02]);
        for _ in 1..16 {
            stream.append_empty_data();
        }
        stream.append_empty_data();

        assert_eq!(MptNode::decode(&stream.out()), None);
    }

    #[test]
    fn decode_rejects_extension_with_invalid_child_length() {
        let mut stream = RlpStream::new_list(2);
        stream.append(&compact_encode(&[0x01, 0x02], false));
        stream.append(&vec![0x01, 0x02]);

        assert_eq!(MptNode::decode(&stream.out()), None);
    }

    #[test]
    fn decode_rejects_short_node_with_invalid_compact_path() {
        let mut stream = RlpStream::new_list(2);
        stream.append(&vec![0x40]);
        stream.append(&b"value".to_vec());

        assert_eq!(MptNode::decode(&stream.out()), None);
    }

    #[test]
    fn node_db_put_returns_hash_of_encoded_node() {
        let mut db = MptNodeDb::new();
        let node = MptNode::branch_with_value(b"branch-value".to_vec());

        let hash = db.put(&node);

        assert_eq!(hash, keccak256(&node.encode()));
        assert!(db.contains(hash));
    }

    #[test]
    fn node_db_get_decodes_stored_node() {
        let mut db = MptNodeDb::new();
        let node = MptNode::extension(vec![0x0a], [0x55u8; 32]);

        let hash = db.put(&node);

        assert_eq!(db.get(hash), Some(node));
    }

    #[test]
    fn node_db_get_encoded_returns_raw_encoded_node() {
        let mut db = MptNodeDb::new();
        let node = MptNode::branch_with_value(b"branch-value".to_vec());
        let encoded = node.encode();

        let hash = db.put(&node);

        assert_eq!(db.get_encoded(hash), Some(encoded.as_slice()));
    }

    #[test]
    fn node_db_deduplicates_identical_nodes_by_hash() {
        let mut db = MptNodeDb::new();
        let node = MptNode::leaf(vec![0x01], b"value".to_vec());

        let first_hash = db.put(&node);
        let second_hash = db.put(&node);

        assert_eq!(first_hash, second_hash);
        assert_eq!(db.len(), 1);
    }

    #[test]
    fn node_db_returns_none_for_missing_hash() {
        let db = MptNodeDb::new();

        assert_eq!(db.get([0x99u8; 32]), None);
        assert_eq!(db.get_encoded([0x99u8; 32]), None);
        assert!(!db.contains([0x99u8; 32]));
    }
}
