use rlp::{DecoderError, Rlp, RlpStream};

use crate::crypto::keccak256;
use crate::mpt::{MptTrie, verify_mpt_proof};
use crate::types::{Address, Hash};

// A minimal Ethereum-like account payload stored as the trie value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub nonce: u64,
    pub balance: u64,
    pub storage_root: Hash,
    pub code_hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountDecodeError {
    InvalidRlp(DecoderError),
    InvalidStorageRootLength(usize),
    InvalidCodeHashLength(usize),
}

impl Account {
    // Create a simple externally owned account with empty storage and code.
    pub fn new_eoa(nonce: u64, balance: u64) -> Self {
        Self {
            nonce,
            balance,
            storage_root: [0; 32],
            code_hash: [0; 32],
        }
    }

    // Encode accounts as RLP so the stored bytes have a deterministic hash.
    pub fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(4);

        stream.append(&self.nonce);
        stream.append(&self.balance);
        stream.append(&self.storage_root.to_vec());
        stream.append(&self.code_hash.to_vec());

        stream.out().to_vec()
    }

    // Decode the exact RLP format produced by Account::encode.
    pub fn try_decode(bytes: &[u8]) -> Result<Self, AccountDecodeError> {
        let rlp = Rlp::new(bytes);

        let nonce: u64 = rlp.val_at(0).map_err(AccountDecodeError::InvalidRlp)?;
        let balance: u64 = rlp.val_at(1).map_err(AccountDecodeError::InvalidRlp)?;

        let storage_root_vec: Vec<u8> = rlp.val_at(2).map_err(AccountDecodeError::InvalidRlp)?;
        let code_hash_vec: Vec<u8> = rlp.val_at(3).map_err(AccountDecodeError::InvalidRlp)?;

        if storage_root_vec.len() != 32 {
            return Err(AccountDecodeError::InvalidStorageRootLength(
                storage_root_vec.len(),
            ));
        }

        if code_hash_vec.len() != 32 {
            return Err(AccountDecodeError::InvalidCodeHashLength(
                code_hash_vec.len(),
            ));
        }

        let mut storage_root = [0u8; 32];
        storage_root.copy_from_slice(&storage_root_vec);

        let mut code_hash = [0u8; 32];
        code_hash.copy_from_slice(&code_hash_vec);

        Ok(Self {
            nonce,
            balance,
            storage_root,
            code_hash,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AccountTrie {
    trie: MptTrie,
}

impl Default for AccountTrie {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountTrie {
    pub fn new() -> Self {
        AccountTrie {
            trie: MptTrie::new(),
        }
    }

    pub fn root_hash(&self) -> Hash {
        self.trie.root_hash()
    }

    // Ethereum account trie keys are keccak256(address), not raw addresses.
    pub fn insert_account(&mut self, address: Address, account: Account) {
        let account_key = keccak256(&address);
        let encode_account = account.encode();

        self.trie.insert(&account_key, encode_account);
    }

    // Load account bytes from the trie and decode them back into Account.
    pub fn get_account(&self, address: Address) -> Option<Account> {
        let account_key = keccak256(&address);
        let encoded_account = self.trie.get(&account_key)?;
        Account::try_decode(&encoded_account).ok()
    }

    // Build an account proof using the hashed account key.
    pub fn prove_account(&self, address: Address) -> Option<Vec<Vec<u8>>> {
        let account_key = keccak256(&address);

        self.trie.prove(&account_key)
    }

    // Recreate the account key and encoded value before verifying the trie proof.
    pub fn verify_account_proof(
        root: Hash,
        address: Address,
        account: &Account,
        proof: &[Vec<u8>],
    ) -> bool {
        let account_key = keccak256(&address);
        let encoded_account = account.encode();

        verify_mpt_proof(root, &account_key, &encoded_account, proof)
    }
}
