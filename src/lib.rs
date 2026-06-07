pub mod account;
pub mod crypto;
pub mod execution;
pub mod mpt;
pub mod storage;
pub mod trie;
pub mod transaction;
pub mod types;

pub use account::{Account, AccountDecodeError, AccountTrie};
pub use crypto::keccak256;
pub use execution::{State, StateError};
pub use mpt::{MptNode, MptNodeDb, MptTrie, Nibble, NodeRef, verify_mpt_proof};
pub use storage::{
    StorageKey, StorageValue, StorageTrie, decode_storage_value, encode_storage_value, storage_trie_key,
};
pub use transaction::{Transaction, TransactionDecodeError, Receipt, ReceiptDecodeError, 
        encode_ordered_trie_index, transaction_root, receipt_root};
pub use trie::{MerkleRadixTrie, verify_proof};
pub use types::{Address, Hash};

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_account_trie() -> (AccountTrie, Address, Account) {
        let mut account_trie = AccountTrie::new();

        let alice: Address = [0x11u8; 20];
        let bob: Address = [0x22u8; 20];
        let carol: Address = [0x33u8; 20];

        let alice_account = Account::new_eoa(1, 1_000);
        let bob_account = Account::new_eoa(2, 2_000);
        let carol_account = Account::new_eoa(3, 3_000);

        account_trie.insert_account(alice, alice_account.clone());
        account_trie.insert_account(bob, bob_account);
        account_trie.insert_account(carol, carol_account);

        (account_trie, alice, alice_account)
    }

    #[test]
    fn account_rlp_round_trips() {
        let account = Account::new_eoa(7, 12_345);
        let encoded = account.encode();
        let decoded = Account::try_decode(&encoded).expect("account should decode");

        assert_eq!(decoded, account);
    }

    #[test]
    fn trie_insert_get_and_missing_key() {
        let mut trie = MerkleRadixTrie::new();

        trie.insert(b"alice", b"1000".to_vec());
        trie.insert(b"bob", b"2000".to_vec());

        assert_eq!(trie.get(b"alice"), Some(b"1000".to_vec()));
        assert_eq!(trie.get(b"bob"), Some(b"2000".to_vec()));
        assert_eq!(trie.get(b"carol"), None);
    }

    #[test]
    fn valid_account_proof_verifies() {
        let (account_trie, alice, alice_account) = sample_account_trie();
        let root = account_trie.root_hash();
        let proof = account_trie
            .prove_account(alice)
            .expect("alice proof should exist");

        assert!(AccountTrie::verify_account_proof(
            root,
            alice,
            &alice_account,
            &proof
        ));
    }

    #[test]
    fn account_trie_uses_mpt_proof_shape() {
        let (account_trie, alice, _) = sample_account_trie();
        let proof = account_trie
            .prove_account(alice)
            .expect("alice proof should exist");

        assert!(
            proof.len() < 65,
            "real MPT proof should be shorter than the old branch-only path"
        );
    }

    #[test]
    fn proof_rejects_wrong_account_value() {
        let (account_trie, alice, _) = sample_account_trie();
        let root = account_trie.root_hash();
        let proof = account_trie
            .prove_account(alice)
            .expect("alice proof should exist");
        let fake_account = Account::new_eoa(1, 999_999);

        assert!(!AccountTrie::verify_account_proof(
            root,
            alice,
            &fake_account,
            &proof
        ));
    }

    #[test]
    fn proof_rejects_wrong_root() {
        let (account_trie, alice, alice_account) = sample_account_trie();
        let proof = account_trie
            .prove_account(alice)
            .expect("alice proof should exist");
        let wrong_root = [0u8; 32];

        assert!(!AccountTrie::verify_account_proof(
            wrong_root,
            alice,
            &alice_account,
            &proof
        ));
    }
}
