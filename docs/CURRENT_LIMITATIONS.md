# Current Limitations

This document records the baseline before replacing the toy trie with a real Merkle Patricia Trie.

## Trie Structure

- The trie only has branch nodes.
- There are no leaf nodes.
- There are no extension nodes.
- There is no hex-prefix compact encoding.
- Every byte key is expanded into nibbles, and the implementation creates one branch level per nibble.
- Proof length is therefore fixed to `key_nibbles + 1`, which is not true for a real MPT.

## Node References

- Every child reference is a 32-byte hash.
- The implementation does not inline small RLP nodes.
- The node database is an in-memory `HashMap<Hash, Vec<u8>>`.
- There is no database trait yet, so trie logic is coupled to the memory backend.

## State Model

- The account trie stores only account values.
- Contract storage tries are not implemented.
- Transaction and receipt tries are not implemented.
- Account `storage_root` and `code_hash` are placeholders in the current demo.

## Operations

- Insert and get are implemented.
- Inclusion proofs are implemented for existing keys.
- Non-inclusion proofs are not implemented.
- Delete is not implemented.
- There is no staged state, rollback, or block-level commit model.

## Error Handling

- Internal decode paths currently use `expect` and `assert_eq`.
- Public-library style `Result` errors are not implemented yet.
- Malformed proof RLP can still panic instead of returning a structured verification error.

## Execution Layer

- There are no transaction types.
- There are no block or header types.
- There is no block processor.
- State transitions are not implemented.

This is acceptable for Milestone 0. Later milestones should replace these limitations intentionally rather than treating the current trie as production MPT logic.
