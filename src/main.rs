use std::collections::HashMap;

use rlp::{Rlp, RlpStream};
use sha3::{Digest, Keccak256};

type Hash = [u8; 32];
type Address = [u8; 20];

// Hash arbitrary bytes with Keccak-256, the hash function used by Ethereum.
fn keccak256(data: &[u8]) -> Hash {
    let digest = Keccak256::digest(data);

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);

    hash
}

// Convert each byte into two 4-bit nibbles so the trie can branch 16 ways.
fn bytes_to_nibbles(bytes: &[u8]) -> Vec<usize> {
    let mut nibbles = Vec::with_capacity(bytes.len() * 2);

    for byte in bytes {
        let high = (byte >> 4) as usize;
        let low = (byte & 0x0f) as usize;

        nibbles.push(high);
        nibbles.push(low);
    }

    nibbles
}

// A minimal Ethereum-like account payload stored as the trie value.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Account {
    nonce: u64,
    balance: u64,
    storage_root: Hash,
    code_hash: Hash,
}

impl Account {
    // Create a simple externally owned account with empty storage and code.
    fn new_eoa(nonce: u64, balance: u64) -> Self {
        Self {
            nonce,
            balance,
            storage_root: [0; 32],
            code_hash: [0; 32],
        }
    }

    // Encode accounts as RLP so the stored bytes have a deterministic hash.
    fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(4);

        stream.append(&self.nonce);
        stream.append(&self.balance);
        stream.append(&self.storage_root.to_vec());
        stream.append(&self.code_hash.to_vec());

        stream.out().to_vec()
    }

    // Decode the exact RLP format produced by Account::encode.
    fn decode(bytes: &[u8]) -> Self {
        let rlp = Rlp::new(bytes);

        let nonce: u64 = rlp.val_at(0).expect("invalid nonce");
        let balance: u64 = rlp.val_at(1).expect("invalid balance");

        let storage_root_vec: Vec<u8> = rlp.val_at(2).expect("invalid storage root");
        let code_hash_vec: Vec<u8> = rlp.val_at(3).expect("invalid code hash");

        assert_eq!(storage_root_vec.len(), 32);
        assert_eq!(code_hash_vec.len(), 32);

        let mut storage_root = [0u8; 32];
        storage_root.copy_from_slice(&storage_root_vec);

        let mut code_hash = [0u8; 32];
        code_hash.copy_from_slice(&code_hash_vec);

        Self {
            nonce,
            balance,
            storage_root,
            code_hash,
        }
    }
}

// A radix branch node has one child slot per nibble plus an optional value.
#[derive(Debug, Clone)]
struct BranchNode {
    children: [Option<Hash>; 16],
    value: Option<Vec<u8>>,
}

impl BranchNode {
    fn new() -> Self {
        BranchNode {
            children: [None; 16],
            value: None,
        }
    }

    // RLP layout: 16 child hash fields followed by the value field.
    fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(17);

        for child in self.children.iter() {
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

        match &self.value {
            Some(value) => {
                stream.append(value);
            }
            None => {
                stream.append_empty_data();
            }
        }

        stream.out().to_vec()
    }

    // Decode a branch node and rebuild empty child slots from empty RLP fields.
    fn decode(bytes: &[u8]) -> Self {
        let rlp = Rlp::new(bytes);

        let item_count = rlp.item_count().expect("invalid branch node rlp");
        assert_eq!(item_count, 17, "branch node must have 17 elements");

        let mut node = BranchNode::new();

        for i in 0..16 {
            let data: Vec<u8> = rlp.val_at(i).expect("invalid child field");

            if data.is_empty() {
                node.children[i] = None;
            } else {
                assert_eq!(data.len(), 32, "child hash must be 32 bytes");

                let mut hash = [0u8; 32];
                hash.copy_from_slice(&data);

                node.children[i] = Some(hash);
            }
        }

        let value: Vec<u8> = rlp.val_at(16).expect("invalid value field");

        if !value.is_empty() {
            node.value = Some(value);
        }

        node
    }
}

// Content-addressed node storage: hash(encoded_node) -> encoded_node.
type NodeDb = HashMap<Hash, Vec<u8>>;

#[derive(Debug, Clone)]
struct MerkleRadixTrie {
    db: NodeDb,
    root: Option<Hash>,
}

impl MerkleRadixTrie {
    fn new() -> Self {
        MerkleRadixTrie {
            db: HashMap::new(),
            root: None,
        }
    }

    // Return the current root hash; an empty trie uses the zero hash sentinel.
    fn root_hash(&self) -> Hash {
        self.root.unwrap_or([0u8; 32])
    }

    // Insert by walking the key nibble by nibble and rebuilding hashes upward.
    fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let nibbles = bytes_to_nibbles(key);

        let new_root = self.insert_at(self.root, &nibbles, &value);

        self.root = Some(new_root);
    }

    fn insert_at(&mut self, node_hash: Option<Hash>, nibbles: &[usize], value: &[u8]) -> Hash {
        // Existing nodes are immutable by hash; decode, modify, then store a new hash.
        let mut node = match node_hash {
            Some(hash) => {
                let encoded_hash = self.db.get(&hash).expect("missing node in db");
                BranchNode::decode(encoded_hash)
            }
            None => BranchNode::new(),
        };

        if nibbles.is_empty() {
            // The full key has been consumed, so the account bytes live here.
            node.value = Some(value.to_vec());
        } else {
            let index = nibbles[0];

            let old_child_hash = node.children[index];

            let new_child_hash = self.insert_at(old_child_hash, &nibbles[1..], value);

            node.children[index] = Some(new_child_hash);
        }

        let encoded_node = node.encode();
        let node_hash = keccak256(&encoded_node);

        // Store the new version of this node under its content hash.
        self.db.insert(node_hash, encoded_node);

        node_hash
    }

    // Look up a value by following the child hash selected by each key nibble.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let nibbles = bytes_to_nibbles(key);

        let mut current_hash = self.root?;

        for nibble in nibbles {
            let encoded_node = self.db.get(&current_hash)?;

            let node = BranchNode::decode(encoded_node);

            current_hash = node.children[nibble]?
        }

        let encoded_node = self.db.get(&current_hash)?;
        let node = BranchNode::decode(encoded_node);

        node.value
    }

    // Return every encoded node on the path from root to the key's value node.
    fn prove(&self, key: &[u8]) -> Option<Vec<Vec<u8>>> {
        let nibbles = bytes_to_nibbles(key);

        let mut proof = Vec::new();
        let mut current_hash = self.root?;
        for nibble in nibbles {
            let encoded_node = self.db.get(&current_hash)?.clone();
            let node = BranchNode::decode(&encoded_node);

            proof.push(encoded_node);
            current_hash = node.children[nibble]?;
        }

        let encoded_node = self.db.get(&current_hash)?.clone();
        proof.push(encoded_node);

        Some(proof)
    }
}

// Verify that a proof connects the given root hash to the expected key/value.
fn verify_proof(root: Hash, key: &[u8], expected_value: &[u8], proof: &[Vec<u8>]) -> bool {
    let nibbles = bytes_to_nibbles(key);

    // This toy trie has one branch node per nibble plus the final value node.
    if proof.len() != nibbles.len() + 1 {
        return false;
    }

    let mut expected_hash = root;
    for depth in 0..proof.len() {
        let encoded_node = &proof[depth];
        let actual_hash = keccak256(encoded_node);

        // Each proof node must hash to the parent reference we expected.
        if expected_hash != actual_hash {
            return false;
        }

        let node = BranchNode::decode(encoded_node);

        if depth == nibbles.len() {
            return node.value.as_deref() == Some(expected_value);
        }

        let nibble = nibbles[depth];
        match node.children[nibble] {
            None => {
                return false;
            }
            Some(child_hash) => {
                // The next proof item must match this child hash.
                expected_hash = child_hash;
            }
        }
    }
    false
}

#[derive(Debug, Clone)]
struct AccountTrie {
    trie: MerkleRadixTrie,
}

impl AccountTrie {
    fn new() -> Self {
        AccountTrie {
            trie: MerkleRadixTrie::new(),
        }
    }

    fn root_hash(&self) -> Hash {
        self.trie.root_hash()
    }

    // Ethereum account trie keys are keccak256(address), not raw addresses.
    fn insert_account(&mut self, address: Address, account: Account) {
        let account_key = keccak256(&address);
        let encode_account = account.encode();

        self.trie.insert(&account_key, encode_account);
    }

    // Load account bytes from the trie and decode them back into Account.
    fn get_account(&self, address: Address) -> Option<Account> {
        let account_key = keccak256(&address);
        let encoded_account = self.trie.get(&account_key)?;
        Some(Account::decode(&encoded_account))
    }

    // Build an account proof using the hashed account key.
    fn prove_account(&self, address: Address) -> Option<Vec<Vec<u8>>> {
        let account_key = keccak256(&address);

        self.trie.prove(&account_key)
    }

    // Recreate the account key and encoded value before verifying the trie proof.
    fn verify_account_proof(
        root: Hash,
        address: Address,
        account: &Account,
        proof: &[Vec<u8>],
    ) -> bool {
        let account_key = keccak256(&address);
        let encoded_account = Account::encode(account);

        verify_proof(root, &account_key, &encoded_account, proof)
    }
}

fn print_address(lable: &str, address: Address) {
    println!("{lable}: 0x{}", hex::encode(address));
}

fn print_hash(lable: &str, hash: Hash) {
    println!("{lable}: 0x{}", hex::encode(hash));
}
fn main() {
    let mut account_trie = AccountTrie::new();

    let alice: Address = [0x11u8; 20];
    let bob: Address = [0x22u8; 20];
    let carol: Address = [0x33u8; 20];

    let alice_account = Account::new_eoa(1, 1_000);
    let bob_account = Account::new_eoa(2, 2_000);
    let carol_account = Account::new_eoa(3, 3_000);

    account_trie.insert_account(alice, alice_account.clone());
    account_trie.insert_account(bob, bob_account.clone());
    account_trie.insert_account(carol, carol_account.clone());

    println!("=== address ===");
    print_address("alice", alice);
    print_address("bob", bob);
    print_address("carol", carol);

    println!();
    println!("=== hashed account keys ===");
    print_hash("keccak256(alice)", keccak256(&alice));
    print_hash("keccak256(bob)", keccak256(&bob));
    print_hash("keccak256(carol)", keccak256(&carol));

    println!();
    println!("=== root ===");
    let root = account_trie.root_hash();
    print_hash("account trie root", root);

    println!();
    println!("=== read account ===");
    let loaded_alice = account_trie
        .get_account(alice)
        .expect("alice account should exist");

    println!("alice account: {:?}", loaded_alice);

    assert_eq!(loaded_alice, alice_account);

    println!();
    println!("=== generate proof ===");
    let proof = account_trie
        .prove_account(alice)
        .expect("proof should exist");

    println!("proof node count: {}", proof.len());

    println!();
    println!("=== verify proof ===");
    let ok = AccountTrie::verify_account_proof(root, alice, &alice_account, &proof);

    println!("valid alice proof: {ok}");

    println!();
    println!("=== fake proof test ===");
    let fake_alice_account = Account::new_eoa(1, 999_999);

    let fake_ok = AccountTrie::verify_account_proof(root, alice, &fake_alice_account, &proof);

    println!("valid fake alice proof: {fake_ok}");
    assert!(ok);
    assert!(!fake_ok);
}

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
        let decoded = Account::decode(&encoded);

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
