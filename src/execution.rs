use std::collections::HashMap;

use crate::storage::{StorageKey, StorageTrie, StorageValue};
use crate::account::{Account, AccountTrie};
use crate::types::{Address, Hash};

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