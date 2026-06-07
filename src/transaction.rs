use std::string::FromUtf8Error;

use rlp::{DecoderError, Rlp, RlpStream};

use crate::mpt::MptTrie;
use crate::types::Address;


pub fn encode_ordered_trie_index(index: usize) -> Vec<u8> {
    let mut stream = RlpStream::new();

    stream.append(&(index as u64));

    stream.out().to_vec()
}

// Transaction tries are keyed by the RLP-encoded transaction index, not by a
// hashed key. Unlike account/storage keys, transaction indexes are deterministic,
// dense, and block-local, so Ethereum keeps their ordered position directly in
// the trie key.
pub fn transaction_root(transactions: &[Transaction]) -> crate::types::Hash {
    let mut trie = MptTrie::new();

    for (index, transaction) in transactions.iter().enumerate() {
        let trie_key = encode_ordered_trie_index(index);
        trie.insert(&trie_key, transaction.encode());
    }

    trie.root_hash()
}

pub fn receipt_root(receipts: &[Receipt]) -> crate::types::Hash {
    let mut trie = MptTrie::new();

    for (index, receipt) in receipts.iter().enumerate() {
        let trie_key = encode_ordered_trie_index(index);
        trie.insert(&trie_key, receipt.encode());
    }

    trie.root_hash()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub from: Address,
    pub to: Address,
    pub nonce: u64,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionDecodeError {
    InvalidRlp(DecoderError),
    InvalidFromLength(usize),
    InvalidToLength(usize),
}

impl Transaction {
    pub fn new_transfer(from: Address, to: Address, nonce: u64, value: u64) -> Self {
        Self { 
            from, 
            to, 
            nonce, 
            value, 
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(4);

        stream.append(&self.from.to_vec());
        stream.append(&self.to.to_vec());
        stream.append(&self.nonce);
        stream.append(&self.value);

        stream.out().to_vec()        
    }

    pub fn try_decode(bytes: &[u8]) -> Result<Self, TransactionDecodeError> {
        let rlp = Rlp::new(bytes);

        let from_vec: Vec<u8> = rlp.val_at(0).map_err(TransactionDecodeError::InvalidRlp)?;
        let to_vec: Vec<u8> = rlp.val_at(1).map_err(TransactionDecodeError::InvalidRlp)?;
        let nonce: u64 = rlp.val_at(2).map_err(TransactionDecodeError::InvalidRlp)?;
        let value: u64 = rlp.val_at(3).map_err(TransactionDecodeError::InvalidRlp)?;

        if from_vec.len() != 20 {
            return Err(TransactionDecodeError::InvalidFromLength(from_vec.len()));
        }        

        if to_vec.len() != 20 {
            return Err(TransactionDecodeError::InvalidToLength(to_vec.len()));
        }

        let mut from = [0u8; 20];
        from.copy_from_slice(&from_vec);

        let mut to = [0u8; 20];
        to.copy_from_slice(&to_vec);

        Ok(
            Self { from, to, nonce, value }
        )
    }
    
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub success: bool,
    pub gas_used: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptDecodeError {
    InvalidRlp(DecoderError),
    InvalidSuccessFlag(u8),
    InvalidErrorUtf8(FromUtf8Error),
}

impl Receipt {
    pub fn success(gas_used: u64) -> Self {
        Self {
            success: true,
            gas_used,
            error: None,
        }
    }

    pub fn failure(gas_used: u64, error: impl Into<String>) -> Self {
        Self {
            success: false,
            gas_used,
            error: Some(error.into()),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut stream = RlpStream::new_list(3);
        let success_flag = u8::from(self.success);
        let error_bytes = self
            .error
            .as_ref()
            .map(|error| error.as_bytes().to_vec())
            .unwrap_or_default();

        stream.append(&success_flag);
        stream.append(&self.gas_used);
        stream.append(&error_bytes);

        stream.out().to_vec()
    }

    pub fn try_decode(bytes: &[u8]) -> Result<Self, ReceiptDecodeError> {
        let rlp = Rlp::new(bytes);

        let success_flag: u8 = rlp.val_at(0).map_err(ReceiptDecodeError::InvalidRlp)?;
        let gas_used: u64 = rlp.val_at(1).map_err(ReceiptDecodeError::InvalidRlp)?;
        let error_bytes: Vec<u8> = rlp.val_at(2).map_err(ReceiptDecodeError::InvalidRlp)?;

        let success = match success_flag {
            0 => false,
            1 => true,
            flag => return Err(ReceiptDecodeError::InvalidSuccessFlag(flag)),
        };

        let error = if error_bytes.is_empty() {
            None
        } else {
            Some(String::from_utf8(error_bytes).map_err(ReceiptDecodeError::InvalidErrorUtf8)?)
        };

        Ok(Self {
            success,
            gas_used,
            error,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_transactions() -> Vec<Transaction> {
        vec![
            Transaction::new_transfer([0x11u8; 20], [0x22u8; 20], 0, 100),
            Transaction::new_transfer([0x22u8; 20], [0x33u8; 20], 1, 200),
        ]
    }

    fn sample_receipts() -> Vec<Receipt> {
        vec![Receipt::success(21_000), Receipt::failure(21_000, "failed")]
    }
    #[test]
    fn ordered_trie_index_encoding_is_deterministic() {
        assert_eq!(encode_ordered_trie_index(7), encode_ordered_trie_index(7));
    }

    #[test]
    fn ordered_trie_index_encoding_distinguishes_positions() {
        assert_ne!(encode_ordered_trie_index(0), encode_ordered_trie_index(1));
        assert_ne!(encode_ordered_trie_index(1), encode_ordered_trie_index(2));
    }

    #[test]
    fn ordered_trie_index_encoding_round_trips_as_rlp_integer() {
        for index in [0usize, 1, 15, 16, 1024] {
            let encoded = encode_ordered_trie_index(index);
            let decoded: u64 = Rlp::new(&encoded).as_val().expect("index should decode");

            assert_eq!(decoded, index as u64);
        }
    }
    #[test]
    fn transfer_transaction_rlp_round_trips() {
        let transaction = Transaction::new_transfer([0x11u8; 20], [0x22u8; 20], 7, 99);

        let decoded =
            Transaction::try_decode(&transaction.encode()).expect("transaction should decode");

        assert_eq!(decoded, transaction);
    }

    #[test]
    fn transaction_decode_rejects_invalid_from_length() {
        let mut stream = RlpStream::new_list(4);
        stream.append(&vec![0x11u8; 19]);
        stream.append(&vec![0x22u8; 20]);
        stream.append(&7u64);
        stream.append(&99u64);

        let result = Transaction::try_decode(&stream.out());

        assert_eq!(result, Err(TransactionDecodeError::InvalidFromLength(19)));
    }

    #[test]
    fn transaction_decode_rejects_invalid_to_length() {
        let mut stream = RlpStream::new_list(4);
        stream.append(&vec![0x11u8; 20]);
        stream.append(&vec![0x22u8; 19]);
        stream.append(&7u64);
        stream.append(&99u64);

        let result = Transaction::try_decode(&stream.out());

        assert_eq!(result, Err(TransactionDecodeError::InvalidToLength(19)));
    }

    #[test]
    fn success_receipt_rlp_round_trips() {
        let receipt = Receipt::success(21_000);

        let decoded = Receipt::try_decode(&receipt.encode()).expect("receipt should decode");

        assert_eq!(decoded, receipt);
    }

    #[test]
    fn failure_receipt_rlp_round_trips() {
        let receipt = Receipt::failure(21_000, "insufficient balance");

        let decoded = Receipt::try_decode(&receipt.encode()).expect("receipt should decode");

        assert_eq!(decoded, receipt);
    }

    #[test]
    fn receipt_decode_rejects_invalid_success_flag() {
        let mut stream = RlpStream::new_list(3);
        stream.append(&2u8);
        stream.append(&21_000u64);
        stream.append(&Vec::<u8>::new());

        let result = Receipt::try_decode(&stream.out());

        assert_eq!(result, Err(ReceiptDecodeError::InvalidSuccessFlag(2)));
    }

    #[test]
    fn receipt_decode_rejects_invalid_error_utf8() {
        let mut stream = RlpStream::new_list(3);
        stream.append(&0u8);
        stream.append(&21_000u64);
        stream.append(&vec![0xff]);

        let result = Receipt::try_decode(&stream.out());

        assert!(matches!(
            result,
            Err(ReceiptDecodeError::InvalidErrorUtf8(_))
        ));
    }

    #[test]
    fn empty_transaction_root_matches_empty_mpt_root() {
        assert_eq!(transaction_root(&[]), MptTrie::new().root_hash());
    }

    #[test]
    fn transaction_root_is_deterministic_for_same_ordered_transactions() {
        let transactions = sample_transactions();

        assert_eq!(transaction_root(&transactions), transaction_root(&transactions));
    }

    #[test]
    fn transaction_root_changes_when_transaction_changes() {
        let transactions = sample_transactions();
        let mut changed_transactions = transactions.clone();
        changed_transactions[0].value += 1;

        assert_ne!(transaction_root(&transactions), transaction_root(&changed_transactions))
    }

    #[test]
    fn transaction_root_changes_when_order_changes() {
        let transactions = sample_transactions();
        let mut reordered_transactions = transactions.clone();
        reordered_transactions.swap(0, 1);

        assert_ne!(
            transaction_root(&transactions),
            transaction_root(&reordered_transactions)
        );
    }

   #[test]
    fn empty_receipt_root_matches_empty_mpt_root() {
        assert_eq!(receipt_root(&[]), MptTrie::new().root_hash());
    }

    #[test]
    fn receipt_root_is_deterministic_for_same_ordered_receipts() {
        let receipts = sample_receipts();

        assert_eq!(receipt_root(&receipts), receipt_root(&receipts));
    }

    #[test]
    fn receipt_root_changes_when_receipt_changes() {
        let receipts = sample_receipts();
        let mut changed_receipts = receipts.clone();
        changed_receipts[0] = Receipt::failure(21_000, "failed");

        assert_ne!(receipt_root(&receipts), receipt_root(&changed_receipts));
    }

    #[test]
    fn receipt_root_changes_when_order_changes() {
        let receipts = sample_receipts();
        let mut reordered_receipts = receipts.clone();
        reordered_receipts.swap(0, 1);

        assert_ne!(receipt_root(&receipts), receipt_root(&reordered_receipts));
    }
}
