# Orphaned and Stale Trie Nodes

This implementation stores trie nodes by hash. Insert operations create new node
versions and return a new root hash instead of mutating old nodes in place.

As a result, an insert can leave older nodes in the node database even when those
nodes are no longer reachable from the latest root.

For example:

- splitting a leaf can replace it with a branch, optionally wrapped by an
  extension;
- splitting an extension can replace the original extension with a shorter
  shared-path extension, a branch, and possibly another extension for the old
  child path.

After such an update, the previous leaf or extension node may still exist in the
database, but the latest root no longer points to it. This is more accurately
described as an orphaned or stale trie node, not as a Rust memory leak.

## Code Example

The orphaned-node behavior comes from creating replacement nodes with `db.put`
and returning the new hash to the caller. The old node hash is not removed from
the database.

For example, when an extension path is split, the old child path may be attached
under a newly created branch:

```rust
let old_remaining = &extension_path[shared_len..];
let old_child_index = old_remaining[0] as usize;

children[old_child_index] = Some(if old_remaining.len() == 1 {
    child
} else {
    self.db
        .put(&MptNode::extension(old_remaining[1..].to_vec(), child))
});

let branch_hash = self.db.put(&MptNode::Branch {
    children,
    value: branch_value,
});

self.wrap_shared_path(&extension_path[..shared_len], branch_hash)
```

`wrap_shared_path` may then create another new extension node above that branch:

```rust
fn wrap_shared_path(&mut self, shared_path: &[Nibble], child: NodeRef) -> Hash {
    if shared_path.is_empty() {
        child
    } else {
        self.db
            .put(&MptNode::extension(shared_path.to_vec(), child))
    }
}
```

The new root eventually points to this newly created structure. The previous
extension node is still stored in the node database, but if no current root path
references its hash, it has become stale.

This behavior is expected for a persistent Merkle trie that keeps historical
roots, because old roots still need their old nodes. If the implementation only
cares about the latest root, then the database needs pruning or garbage
collection to remove nodes that are unreachable from that root.
