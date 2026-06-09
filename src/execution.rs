use std::collections::HashMap;

use rlp::{DecoderError, Rlp, RlpStream};

use crate::Receipt;
use crate::account::{Account, AccountTrie};
use crate::crypto::keccak256;
use crate::mpt::MptNodeDb;
use crate::storage::{StorageKey, StorageTrie, StorageValue};
use crate::transaction::{Transaction, TransactionDecodeError, transaction_root, receipt_root};
use crate::types::{Address, Hash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub parent_hash: Hash,
    pub number: u64,
    pub state_root: Hash,
    pub transactions_root: Hash,
    pub receipts_root: Hash,
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderDecodeError {
    InvalidRlp(DecoderError),
    InvalidParentHashLength(usize),
    InvalidStateRootLength(usize),
    InvalidTransactionsRootLength(usize),
    InvalidReceiptsRootLength(usize),
}

impl Header {
    pub fn new(
        parent_hash: Hash,
        number: u64,
        state_root: Hash,
        transactions_root: Hash,
        receipts_root: Hash,
        timestamp: u64,
    ) -> Self{
        Self { 
            parent_hash, 
            number, 
            state_root, 
            transactions_root, 
            receipts_root, 
            timestamp, 
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(6);

        stream.append(&self.parent_hash.to_vec());
        stream.append(&self.number);
        stream.append(&self.state_root.to_vec());
        stream.append(&self.transactions_root.to_vec());
        stream.append(&self.receipts_root.to_vec());
        stream.append(&self.timestamp);

        stream.out().to_vec()
    }

    pub fn hash(&self) -> Hash {
        keccak256(&self.encode())
    }

    pub fn try_decode(bytes: &[u8]) -> Result<Self, HeaderDecodeError> {
        let rlp = Rlp::new(bytes);

        let parent_hash_vec: Vec<u8> = rlp.val_at(0).map_err(HeaderDecodeError::InvalidRlp)?;
        let number: u64 = rlp.val_at(1).map_err(HeaderDecodeError::InvalidRlp)?;
        let state_root_vec: Vec<u8> = rlp.val_at(2).map_err(HeaderDecodeError::InvalidRlp)?;
        let transactions_root_vec: Vec<u8> =
            rlp.val_at(3).map_err(HeaderDecodeError::InvalidRlp)?;
        let receipts_root_vec: Vec<u8> = rlp.val_at(4).map_err(HeaderDecodeError::InvalidRlp)?;
        let timestamp: u64 = rlp.val_at(5).map_err(HeaderDecodeError::InvalidRlp)?;

        Ok(Self { 
            parent_hash: decode_hash(parent_hash_vec, HeaderDecodeError::InvalidParentHashLength)?, 
            number, 
            state_root: decode_hash(state_root_vec, HeaderDecodeError::InvalidStateRootLength)?, 
            transactions_root: decode_hash(
                transactions_root_vec, 
                HeaderDecodeError::InvalidTransactionsRootLength,
            )?,
            receipts_root: decode_hash(
                receipts_root_vec, 
                HeaderDecodeError::InvalidReceiptsRootLength,
            )?, 
            timestamp 
        })
    }
}

fn decode_hash (
    bytes: Vec<u8>,
    error: impl FnOnce(usize) -> HeaderDecodeError,
) -> Result<Hash, HeaderDecodeError> {
    if bytes.len() != 32 {
        return Err(error(bytes.len()));
    }

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    
    Ok(hash)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockDecodeError {
    InvalidRlp(DecoderError),
    InvalidHeader(HeaderDecodeError),
    InvalidTransaction {
        index: usize,
        error: TransactionDecodeError,
    },
}

impl Block {
    pub fn new(header: Header, transactions: Vec<Transaction>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(2);

        stream.append(&self.header.encode());
        stream.begin_list(self.transactions.len());
        for transaction in &self.transactions {
            stream.append(&transaction.encode());
        }

        stream.out().to_vec()
    }

    pub fn hash(&self) -> Hash {
        keccak256(&self.encode())
    }

    pub fn try_decode(bytes: &[u8]) -> Result<Self, BlockDecodeError> {
        let rlp = Rlp::new(bytes);

        let encoded_header: Vec<u8> = rlp.val_at(0).map_err(BlockDecodeError::InvalidRlp)?;
        let header = 
            Header::try_decode(&encoded_header).map_err(BlockDecodeError::InvalidHeader)?;
        let transactions_rlp = rlp.at(1).map_err(BlockDecodeError::InvalidRlp)?;
        let transaction_count = transactions_rlp
            .item_count()
            .map_err(BlockDecodeError::InvalidRlp)?;
        let mut transactions = Vec::with_capacity(transaction_count);

        for index in 0..transaction_count {
            let encoded_transaction: Vec<u8> = transactions_rlp
                .val_at(index)
                .map_err(BlockDecodeError::InvalidRlp)?;
            let transaction = Transaction::try_decode(&encoded_transaction)
                .map_err(|error| BlockDecodeError::InvalidTransaction { index, error })?;
            transactions.push(transaction);
        }

        Ok(Self { 
            header, 
            transactions 
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub post_state_root: Hash,
    pub receipts: Vec<Receipt>,
    pub transactions_root: Hash,
    pub receipts_root: Hash,
}

impl ExecutionResult {
    pub fn new(
        post_state_root: Hash,
        transactions: &[Transaction],
        receipts: Vec<Receipt>,
    ) -> Self {
        let transactions_root = transaction_root(transactions);
        let receipts_root = receipt_root(&receipts);
        Self { 
            post_state_root, 
            receipts, 
            transactions_root, 
            receipts_root, 
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    MissingSender(Address),
    MissingRecipient(Address),
    InvalidNonce {
        address: Address,
        expected: u64,
        actual: u64,
    },
    InsufficientBalance {
        address: Address,
        balance: u64,
        required: u64,
    },
    State(StateError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    AccountNotFound(Address),
    StorageTrieUnavailable(Address)
}

#[derive(Debug, Clone, Default)]
pub struct State {
    accounts: AccountTrie,
    storage_tries: HashMap<Address, StorageTrie>,
}

impl State {
    pub fn new() -> Self {
        Self { 
            accounts: AccountTrie::new(),
            storage_tries: HashMap::new(),
        }
    }

    pub fn from_account_root(db: MptNodeDb, root: Hash) -> Self {
        Self {
            accounts: AccountTrie::from_root(db, root),
            storage_tries: HashMap::new(),
        }
    }

    pub fn into_account_parts(self) -> (MptNodeDb, Option<Hash>) {
        self.accounts.into_parts()
    }

    pub fn root_hash(&self) -> Hash {
        self.accounts.root_hash()
    }
    pub fn create_account(&mut self, address: Address, account: Account) {
        self.sync_storage_trie(address, &account);
        self.accounts.insert_account(address, account);
    }
    pub fn update_account(&mut self, address: Address, account: Account) {
        self.sync_storage_trie(address, &account);
        self.accounts.insert_account(address, account);
    }
    pub fn get_account(&self, address: Address) -> Option<Account> {
        self.accounts.get_account(address)
    }
    pub fn prove_account(&self, address: Address) -> Option<Vec<Vec<u8>>> {
        self.accounts.prove_account(address)
    }
    pub fn verify_account_proof(
        root: Hash,
        address: Address,
        account: &Account,
        proof: &[Vec<u8>],
    ) -> bool {
        AccountTrie::verify_account_proof(root, address, account, proof)
    }

    pub fn get_storage_slot(&self, address: Address, slot_key: StorageKey) -> Option<StorageValue>{
        self.storage_tries
            .get(&address)
            .and_then(|storage_trie| storage_trie.get_slot(slot_key))
    }

    pub fn set_storage_slot(
        &mut self, 
        address: Address, 
        slot_key: StorageKey, 
        value: StorageValue
    ) -> Result<(), StateError> {
        let mut account = self
            .get_account(address)
            .ok_or(StateError::AccountNotFound(address))?;

        let storage_trie = self.storage_trie_mut(address, &account)?;

        storage_trie.set_slot(slot_key, value);
        account.storage_root = storage_trie.root_hash();
        self.update_account(address, account);

        Ok(())

    }

    fn storage_trie_mut(&mut self, address: Address, account: &Account) -> Result<&mut StorageTrie, StateError> {
        if self.storage_tries.contains_key(&address) {
            return Ok(
                self.storage_tries
                .get_mut(&address)
                .expect("storage trie existence was checked")
            )
        }

        if account.storage_root != Account::empty_storage_root() {
            return Err(StateError::StorageTrieUnavailable(address));
        }

        Ok(self
            .storage_tries
            .entry(address)
            .or_insert_with(StorageTrie::new)
        )
    }

    fn sync_storage_trie(&mut self, address: Address, account: &Account) {
        match self.storage_tries.get(&address) {
            Some(storage_trie) if storage_trie.root_hash() == account.storage_root => {}
            _ => {self.storage_tries.remove(&address);}
        }
    }
}

#[cfg(test)]
mod tests {
use super::*;

    fn sample_header() -> Header {
        Header::new(
            [0x11u8; 32],
            7,
            [0x22u8; 32],
            [0x33u8; 32],
            [0x44u8; 32],
            1_700_000_000,
        )
    }

    fn sample_transactions() -> Vec<Transaction> {
        vec![
            Transaction::new_transfer([0x11u8; 20], [0x22u8; 20], 0, 100),
            Transaction::new_transfer([0x22u8; 20], [0x33u8; 20], 1, 200),
        ]
    }

    fn sample_block() -> Block {
        Block::new(sample_header(), sample_transactions())
    }

    fn sample_receipts() -> Vec<Receipt> {
        vec![Receipt::success(21_000), Receipt::failure(21_000, "failed")]
    }

    #[test]
    fn header_rlp_round_trips() {
        let header = sample_header();

        let decoded = Header::try_decode(&header.encode()).expect("header should decode");

        assert_eq!(decoded, header);
    }

    #[test]
    fn header_hash_is_deterministic() {
        let header = sample_header();

        assert_eq!(header.hash(), header.hash());
        assert_eq!(header.hash(), keccak256(&header.encode()));
    }

    #[test]
    fn header_hash_changes_when_root_changes() {
        let header = sample_header();
        let mut changed_header = header.clone();
        changed_header.state_root = [0x55u8; 32];

        assert_ne!(header.hash(), changed_header.hash());
    }

    #[test]
    fn header_decode_rejects_invalid_root_length() {
        let mut stream = RlpStream::new_list(6);
        stream.append(&vec![0x11u8; 32]);
        stream.append(&7u64);
        stream.append(&vec![0x22u8; 31]);
        stream.append(&vec![0x33u8; 32]);
        stream.append(&vec![0x44u8; 32]);
        stream.append(&1_700_000_000u64);

        let result = Header::try_decode(&stream.out());

        assert_eq!(result, Err(HeaderDecodeError::InvalidStateRootLength(31)));
    }

    #[test]
    fn block_rlp_round_trips() {
        let block = sample_block();

        let decoded = Block::try_decode(&block.encode()).expect("block should decode");

        assert_eq!(decoded, block);
    }

    #[test]
    fn block_hash_is_deterministic() {
        let block = sample_block();

        assert_eq!(block.hash(), block.hash());
        assert_eq!(block.hash(), keccak256(&block.encode()));
    }

    #[test]
    fn block_hash_changes_when_transaction_changes() {
        let block = sample_block();
        let mut changed_block = block.clone();
        changed_block.transactions[0].value += 1;

        assert_ne!(block.hash(), changed_block.hash());
    }

    #[test]
    fn block_decode_reports_invalid_transaction_index() {
        let mut invalid_transaciton = RlpStream::new_list(4);
        invalid_transaciton.append(&vec![0x11u8; 19]);
        invalid_transaciton.append(&vec![0x22u8; 20]);
        invalid_transaciton.append(&0u64);
        invalid_transaciton.append(&100u64);

        let mut transaction_list = RlpStream::new_list(1);
        transaction_list.append(&invalid_transaciton.out().to_vec());

        let mut block_stream = RlpStream::new_list(2);
        block_stream.append(&sample_header().encode());
        block_stream.append_raw(&transaction_list.out(), 1);

        let result = Block::try_decode(&block_stream.out());

        assert_eq!(
            result,
            Err(BlockDecodeError::InvalidTransaction { 
                index: 0, 
                error: TransactionDecodeError::InvalidFromLength(19) 
            })
        );
    }
    
    #[test]
    fn execution_result_derives_transaction_and_receipt_roots() {
        let transactions = sample_transactions();
        let receipts = sample_receipts();
        let post_state_root = [0x55u8; 32];

        let result = ExecutionResult::new(post_state_root, &transactions, receipts.clone());

        assert_eq!(result.post_state_root, post_state_root);
        assert_eq!(result.receipts, receipts);
        assert_eq!(result.transactions_root, transaction_root(&transactions));
        assert_eq!(result.receipts_root, receipt_root(&result.receipts));
    }

    #[test]
    fn execution_result_roots_change_when_inputs_change() {
        let transactions = sample_transactions();
        let mut changed_transactions = transactions.clone();
        changed_transactions[0].value += 1;
        let receipts = sample_receipts();

        let result = ExecutionResult::new([0x55u8; 32], &transactions, receipts.clone());
        let changed_result = ExecutionResult::new([0x55u8; 32], &changed_transactions, receipts);

        assert_ne!(result.transactions_root, changed_result.transactions_root);
        assert_eq!(result.receipts_root, changed_result.receipts_root);
    }

    #[test]
    fn execution_error_carries_context() {
        let address = [0x11u8; 20];

        let error = ExecutionError::InvalidNonce {
            address,
            expected: 7,
            actual: 6,
        };

        assert_eq!(
            error,
            ExecutionError::InvalidNonce {
                address,
                expected: 7,
                actual: 6,
            }
        );
    }

    #[test]
    fn empty_state_has_empty_account_root() {
        let state = State::new();

        assert_eq!(state.root_hash(), AccountTrie::new().root_hash());
    }
    
    #[test]
    fn state_creates_and_loads_account() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let account = Account::new_eoa(1, 100);

        state.create_account(address, account.clone());

        assert_eq!(state.get_account(address), Some(account));
    }

    #[test]
    fn state_reopens_accounts_from_saved_root_and_database() {
        let mut state = State::new();
        let alice = [0x11u8; 20];
        let bob = [0x22u8; 20];
        let alice_account = Account::new_eoa(1, 100);
        let bob_account = Account::new_eoa(2, 200);

        state.create_account(alice, alice_account.clone());
        state.create_account(bob, bob_account.clone());
        let root = state.root_hash();
        let (db, saved_root) = state.into_account_parts();

        let reopened = State::from_account_root(db, root);

        assert_eq!(saved_root, Some(root));
        assert_eq!(reopened.root_hash(), root);
        assert_eq!(reopened.get_account(alice), Some(alice_account));
        assert_eq!(reopened.get_account(bob), Some(bob_account));
        assert_eq!(reopened.get_account([0x33u8; 20]), None);
    }
    
    #[test]
    fn updating_account_changes_state_root() {
        let mut state = State::new();
        let address = [0x11u8; 20];

        state.create_account(address, Account::new_eoa(1, 100));
        let first_root = state.root_hash();
        state.update_account(address, Account::new_eoa(1, 200));

        assert_ne!(state.root_hash(), first_root);
    }

    #[test]
    fn writing_same_account_keeps_state_root_stable() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let account = Account::new_eoa(1, 100);

        state.create_account(address, account.clone());
        let first_root = state.root_hash();
        state.update_account(address, account);

        assert_eq!(state.root_hash(), first_root);
    }

    #[test]
    fn state_account_proof_verifies() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let account = Account::new_eoa(1, 100);

        state.create_account(address, account.clone());
        let proof = state
            .prove_account(address)
            .expect("account proof should exist");

        assert!(State::verify_account_proof(
            state.root_hash(), 
            address, 
            &account,
            &proof
        ));
    }

    #[test]
    fn state_sets_and_reads_storage_slot() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let slot_key = [0x22u8; 32];

        state.create_account(address, Account::new_eoa(1, 100));
        state
            .set_storage_slot(address, slot_key, b"value".to_vec())
            .expect("storage write should succeed");

        assert_eq!(
            state.get_storage_slot(address, slot_key),
            Some(b"value".to_vec())
        );
    }

    #[test]
    fn storage_write_updates_account_and_state_roots() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let slot_key = [0x22u8; 32];

        state.create_account(address, Account::new_eoa(1, 100));
        let old_state_root = state.root_hash();
        let old_storage_root = state.get_account(address).unwrap().storage_root;

        state
            .set_storage_slot(address, slot_key, b"value".to_vec())
            .expect("storage write should succeed");

        let updated_account = state.get_account(address).unwrap();
        assert_ne!(updated_account.storage_root, old_storage_root);
        assert_ne!(state.root_hash(), old_state_root);
    }

    #[test]
    fn writing_same_storage_value_keeps_state_root_stable() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let slot_key = [0x22u8; 32];

        state.create_account(address, Account::new_eoa(1, 100));
        state
            .set_storage_slot(address, slot_key, b"value".to_vec())
            .expect("storage write should succeed");
        let first_root = state.root_hash();

        state
            .set_storage_slot(address, slot_key, b"value".to_vec())
            .expect("storage write should succeed");

        assert_eq!(state.root_hash(), first_root);
    }

    #[test]
    fn storage_write_preserves_account_fields() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let slot_key = [0x22u8; 32];
        let code_hash = [0x33u8; 32];
        let account = Account::new_contract(7, 99, Account::empty_storage_root(), code_hash);

        state.create_account(address, account);
        state
            .set_storage_slot(address, slot_key, b"value".to_vec())
            .expect("storage write should succeed");

        let updated_account = state.get_account(address).unwrap();
        assert_eq!(updated_account.nonce, 7);
        assert_eq!(updated_account.balance, 99);
        assert_eq!(updated_account.code_hash, code_hash);
    }

    #[test]
    fn storage_write_requires_existing_account() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let slot_key = [0x22u8; 32];

        let result = state.set_storage_slot(address, slot_key, b"value".to_vec());

        assert_eq!(result, Err(StateError::AccountNotFound(address)));
    }

    #[test]
    fn storage_write_rejects_unknown_non_empty_storage_root() {
        let mut state = State::new();
        let address = [0x11u8; 20];
        let slot_key = [0x22u8; 32];
        let account = Account::new_contract(7, 99, [0x44u8; 32], [0x33u8; 32]);

        state.create_account(address, account);
        let result = state.set_storage_slot(address, slot_key, b"value".to_vec());

        assert_eq!(result, Err(StateError::StorageTrieUnavailable(address)));
    }
}