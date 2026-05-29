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
        (true, true) => 4,
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
    let flag = *&nibbles.first()?;
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
}