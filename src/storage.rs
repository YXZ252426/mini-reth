use rlp::{Rlp, RlpStream};

use crate::crypto::keccak256;
use crate::mpt::{MptNodeDb, MptTrie, verify_mpt_proof};
use crate::types::Hash;

pub type StorageKey = [u8; 32];
pub type StorageValue = Vec<u8>;

pub fn storage_trie_key(slot_key: &StorageKey) -> Hash {
    keccak256(slot_key)
}

pub fn encode_storage_value(value: &[u8]) -> Vec<u8> {
    let mut stream = RlpStream::new();
    stream.append(&value);
    stream.out().to_vec()
}

pub fn decode_storage_value( encoded: &[u8]) -> Option<StorageValue> {
    Rlp::new(encoded).as_val().ok()
}

#[derive(Debug, Clone, Default)]
pub struct StorageTrie {
    trie: MptTrie,
}

impl StorageTrie {
    pub fn new() -> Self {
        Self {
            trie: MptTrie::new(),
        }
    }
    
    pub fn from_root(db: MptNodeDb, root: Hash) -> Self {
        Self {
            trie: MptTrie::from_root(db, root),
        }
    }

    pub fn into_parts(self) -> (MptNodeDb, Option<Hash>) {
        self.trie.into_parts()
    }

    pub fn root_hash(&self) -> Hash {
        self.trie.root_hash()
    }

    pub fn get_slot(&self, slot_key: StorageKey) -> Option<StorageValue> {
        let trie_key = storage_trie_key(&slot_key);
        let encoded_value = self.trie.get(&trie_key)?;

        decode_storage_value(&encoded_value)
    }

    pub fn set_slot(&mut self, slot_key: StorageKey, value: StorageValue) {
        let trie_key = storage_trie_key(&slot_key);
        let encoded_value = encode_storage_value(&value);
        self.trie.insert(&trie_key, encoded_value);
    }

    pub fn prove_slot(&self, slot_key: StorageKey) -> Option<Vec<Vec<u8>>> {
        let trie_key = storage_trie_key(&slot_key);
        self.trie.prove(&trie_key)
    }

    pub fn verify_slot_proof(
        root: Hash,
        slot_key: StorageKey,
        value: &[u8],
        proof: &[Vec<u8>],
    ) -> bool {
        let trie_key = storage_trie_key(&slot_key);
        let encoded_value = encode_storage_value(value);

        verify_mpt_proof(root, &trie_key, &encoded_value, proof)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_trie_key_is_deterministic() {
        let slot_key = [0x11u8; 32];

        assert_eq!(storage_trie_key(&slot_key), storage_trie_key(&slot_key));
    }

    #[test]
    fn different_storage_keys_hash_differently() {
        let first_key = [0x11u8; 32];
        let second_key = [0x22u8; 32];

        assert_ne!(storage_trie_key(&first_key), storage_trie_key(&second_key));
    }

    #[test]
    fn storage_value_rlp_round_trips() {
        let value = b"slot-value".to_vec();
        let encoded = encode_storage_value(&value);

        assert_eq!(decode_storage_value(&encoded), Some(value));
    }

    #[test]
    fn empty_storage_value_rlp_round_trips() {
        let value = Vec::new();
        let encoded = encode_storage_value(&value);

        assert_eq!(decode_storage_value(&encoded), Some(value));
    }

    #[test]
    fn decode_storage_value_rejects_invalid_rlp() {
        assert_eq!(decode_storage_value(&[0xff]), None);
    }

    #[test]
    fn empty_storage_trie_has_zero_root_and_no_slots() {
        let trie = StorageTrie::new();

        assert_eq!(trie.root_hash(), [0u8; 32]);
        assert_eq!(trie.get_slot([0x11u8; 32]), None);
    }

    #[test]
    fn storage_trie_sets_and_gets_slot() {
        let mut trie = StorageTrie::new();
        let slot_key = [0x11u8; 32];

        trie.set_slot(slot_key, b"value".to_vec());
        assert_ne!(trie.root_hash(), [0u8; 32]);
        assert_eq!(trie.get_slot(slot_key), Some(b"value".to_vec()));
    }

    #[test]
    fn storage_trie_overwrites_slot() {
        let mut trie = StorageTrie::new();
        let slot_key = [0x11u8; 32];

        trie.set_slot(slot_key, b"old".to_vec());
        let old_hash = trie.root_hash();
        trie.set_slot(slot_key, b"new".to_vec());

        assert_ne!(trie.root_hash(), old_hash);
        assert_eq!(trie.get_slot(slot_key), Some(b"new".to_vec())); 
    }

    #[test]
    fn storage_trie_same_slot_same_value_keeps_root_stable() {
        let mut trie = StorageTrie::new();
        let slot_key = [0x11u8; 32];

        trie.set_slot(slot_key, b"value".to_vec());
        let first_root = trie.root_hash();
        trie.set_slot(slot_key, b"value".to_vec());

        assert_eq!(trie.root_hash(), first_root);
    }

    #[test]
    fn storage_trie_stores_empty_value_as_regular_value() {
        let mut trie = StorageTrie::new();
        let slot_key = [0x11u8; 32];

        trie.set_slot(slot_key, Vec::new());

        assert_eq!(trie.get_slot(slot_key), Some(Vec::new()));
    }

    #[test]
    fn storage_slot_proof_verifies() {
        let mut trie = StorageTrie::new();
        let slot_key = [0x11u8; 32];
        trie.set_slot(slot_key, b"value".to_vec());
        let proof = trie.prove_slot(slot_key).expect("slot proof should exist");

        assert!(StorageTrie::verify_slot_proof(
            trie.root_hash(),
            slot_key,
            b"value",
            &proof
        ));
    }

    #[test]
    fn storage_slot_proof_rejects_fake_value() {
        let mut trie = StorageTrie::new();
        let slot_key = [0x11u8; 32];
        trie.set_slot(slot_key, b"value".to_vec());
        let proof = trie.prove_slot(slot_key).expect("slot proof should exist");

        assert!(!StorageTrie::verify_slot_proof(
            trie.root_hash(),
            slot_key,
            b"fake",
            &proof
        ));
    }

    #[test]
    fn storage_slot_proof_missing_slot_returns_none() {
        let mut trie = StorageTrie::new();

        trie.set_slot([0x11u8; 32], b"value".to_vec());

        assert_eq!(trie.prove_slot([0x22u8; 32]), None);
    }

    #[test]
    fn storage_trie_reopens_from_saved_root_and_database() {
        let mut trie = StorageTrie::new();
        let first_key = [0x11u8; 32];
        let second_key = [0x22u8; 32];

        trie.set_slot(first_key, b"first".to_vec());
        trie.set_slot(second_key, b"second".to_vec());
        let root = trie.root_hash();
        let (db, saved_root) = trie.into_parts();

        let reopened = StorageTrie::from_root(db, root);

        assert_eq!(saved_root, Some(root));
        assert_eq!(reopened.root_hash(), root);
        assert_eq!(reopened.get_slot(first_key), Some(b"first".to_vec()));
        assert_eq!(reopened.get_slot(second_key), Some(b"second".to_vec()));
        assert_eq!(reopened.get_slot([0x33u8; 32]), None);
    }
}
