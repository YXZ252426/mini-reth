# RLP Notes

RLP means Recursive Length Prefix. It is a compact binary encoding used by
Ethereum for byte strings and nested lists. RLP does not know about account
fields, trie nodes, addresses, hashes, or transactions. It only knows two data
shapes:

- a byte string;
- a list of other RLP values.

Everything else is a convention built on top of those two shapes. For example,
an account is encoded as a list of four fields, and an MPT branch node is
encoded as a list of seventeen fields.

## 1. The Core Model

Think of RLP as a way to encode a tree:

```text
value =
  bytes
  or
  list(value, value, ...)
```

This is why the word "recursive" appears in the name. A list can contain byte
strings, other lists, or deeply nested combinations.

RLP is also intentionally schema-free. The encoded bytes do not contain field
names or Rust types. If the decoder sees:

```text
[0x01, 0x02, 0x03]
```

it does not know whether that means:

- three small integers;
- three one-byte strings;
- a short path;
- part of a trie node;
- something invalid for the caller's schema.

The caller must know the expected structure.

## 2. Encoding Rules

RLP has separate rules for byte strings and lists.

### Byte Strings

A single byte below `0x80` is encoded as itself:

```text
0x7f -> 0x7f
```

A byte string with length from 0 to 55 bytes is encoded as:

```text
0x80 + length, followed by the bytes
```

Examples:

```text
""      -> 0x80
"cat"   -> 0x83 0x63 0x61 0x74
"dog"   -> 0x83 0x64 0x6f 0x67
```

For byte strings longer than 55 bytes, RLP uses a long-form prefix:

```text
0xb7 + length_of_length, followed by length, followed by the bytes
```

In this toy project most values are short enough that the short-form rule is
the one you will usually see, except for larger trie nodes or hashes embedded
inside bigger structures.

### Lists

A list whose payload is 0 to 55 bytes is encoded as:

```text
0xc0 + payload_length, followed by the encoded child values
```

The payload length is not the number of list items. It is the number of encoded
bytes inside the list.

Example:

```text
["cat", "dog"]
```

Each child is encoded first:

```text
"cat" -> 0x83 0x63 0x61 0x74
"dog" -> 0x83 0x64 0x6f 0x67
```

The payload is 8 bytes:

```text
0x83 0x63 0x61 0x74 0x83 0x64 0x6f 0x67
```

So the list prefix is:

```text
0xc0 + 8 = 0xc8
```

Final encoding:

```text
0xc8 0x83 0x63 0x61 0x74 0x83 0x64 0x6f 0x67
```

For lists with payloads longer than 55 bytes, RLP uses:

```text
0xf7 + length_of_length, followed by payload_length, followed by payload
```

## 3. `RlpStream` Mental Model

`RlpStream` is a builder for RLP bytes.

Use it like this:

1. Declare the shape you are building.
2. Append exactly the expected number of items.
3. Call `out()` to get the final bytes.

The important point is that list length means item count at the API level, but
payload length at the RLP byte level.

```rust
use rlp::RlpStream;

let mut stream = RlpStream::new_list(2);
stream.append(&"cat");
stream.append(&"dog");

let out = stream.out();
```

The API call says "this list has two items." The final bytes say "this list has
eight payload bytes."

## 4. `RlpStream::new()`

`RlpStream::new()` creates an empty stream without immediately declaring a list.
Use it when the top-level value is a single byte string or scalar value.

This project uses it in storage value encoding:

```rust
let mut stream = RlpStream::new();
stream.append(&value);
stream.out().to_vec()
```

The top-level encoded value is just `value`, not `[value]`.

That difference matters:

```text
RLP(value)     != RLP([value])
```

## 5. `RlpStream::new_list(len)`

`new_list(len)` creates a stream whose top-level value is a list with exactly
`len` items.

Example:

```rust
let mut stream = RlpStream::new_list(4);
stream.append(&nonce);
stream.append(&balance);
stream.append(&storage_root.to_vec());
stream.append(&code_hash.to_vec());
let encoded_account = stream.out().to_vec();
```

This is the account shape used by this project:

```text
[nonce, balance, storage_root, code_hash]
```

The encoded bytes do not contain those field names. Decode code must read the
same positions back in the same order.

## 6. `RlpStream::begin_list(len)`

`begin_list(len)` appends a nested list to the current stream.

This example:

```rust
use rlp::RlpStream;

let mut stream = RlpStream::new_list(2);
stream.begin_list(2).append(&"cat").append(&"dog");
stream.append(&"");
let out = stream.out();
```

encodes this structure:

```text
[
  ["cat", "dog"],
  ""
]
```

The first call declares the outer list:

```rust
RlpStream::new_list(2)
```

The next call appends the first outer item, which is itself a list:

```rust
stream.begin_list(2).append(&"cat").append(&"dog");
```

The final append adds the second outer item:

```rust
stream.append(&"");
```

The bytes are:

```text
0xca                                outer list, 10 payload bytes
  0xc8                              inner list, 8 payload bytes
    0x83 0x63 0x61 0x74             "cat"
    0x83 0x64 0x6f 0x67             "dog"
  0x80                              empty byte string
```

So:

```text
0xc8 = 0xc0 + 8
0xca = 0xc0 + 10
0x83 = 0x80 + 3
0x80 = empty byte string
```

`begin_list` is useful when the schema contains a list inside a list. The
current project mostly uses fixed top-level lists, so `new_list` appears more
often than `begin_list`.

## 7. `append`

`append` encodes a Rust value as one RLP item and adds it to the stream.

Examples:

```rust
stream.append(&self.nonce);
stream.append(&self.balance);
stream.append(&self.storage_root.to_vec());
stream.append(&value);
```

The exact encoding depends on the type's `Encodable` implementation.

Important examples:

- `u64` is encoded as a minimal big-endian integer byte string.
- `Vec<u8>` is encoded as a byte string.
- `&[u8]` is encoded as a byte string.
- `String` and `&str` are encoded as byte strings.

RLP itself does not have a separate integer type. Integers become byte strings
using Ethereum's integer convention.

For example:

```text
0      -> 0x80
15     -> 0x0f
1024   -> 0x82 0x04 0x00
```

The integer `0` and the empty byte string both have the same RLP bytes:

```text
0x80
```

The meaning comes from the type requested during decoding.

## 8. `append_empty_data`

`append_empty_data()` appends an empty byte string:

```text
0x80
```

This project uses it for missing trie children and missing branch values:

```rust
stream.append_empty_data();
```

In an MPT branch node, a missing child is not encoded as a Rust `None` value.
It is encoded as empty bytes. The schema says:

```text
branch = [
  child_0,
  child_1,
  ...
  child_15,
  value
]
```

Each missing child slot is represented by `0x80`.

## 9. `append_raw`

`append_raw` is for cases where you already have RLP-encoded bytes and want to
insert them into a stream without encoding them again.

Use it carefully. These two are different:

```rust
stream.append(&already_rlp_encoded_bytes);
stream.append_raw(&already_rlp_encoded_bytes, 1);
```

The first one treats the bytes as ordinary data and wraps them as an RLP byte
string. The second one treats the bytes as already-encoded RLP.

In this project, child references are currently stored as 32-byte hashes rather
than inline encoded nodes. Because of that, normal `append` is enough:

```rust
let child_bytes: &[u8] = child;
stream.append(&child_bytes);
```

If the project later adds Ethereum's inline small-node references, `append_raw`
may become relevant because inline child nodes are already RLP-encoded nodes.

## 10. `Rlp`

`Rlp` is the decoding-side view over encoded bytes.

```rust
use rlp::Rlp;

let rlp = Rlp::new(encoded);
```

It does not copy or fully decode everything immediately. It gives structured
access to the encoded value.

Common methods used in this project:

```rust
rlp.item_count()
rlp.val_at(index)
rlp.as_val()
```

## 11. `item_count`

`item_count()` returns the number of items if the current RLP value is a list.

This project uses it to identify trie node shape:

```rust
let item_count = rlp.item_count().ok()?;

match item_count {
    2 => decode_leaf_or_extension(...),
    17 => decode_branch(...),
    _ => None,
}
```

This works because the project schema says:

- leaf node: list of 2;
- extension node: list of 2;
- branch node: list of 17.

RLP does not know the difference between a leaf and an extension. The code must
inspect the compact-encoded path to decide that.

## 12. `val_at`

`val_at(index)` decodes one list item at a given position into the requested
Rust type.

```rust
let nonce: u64 = rlp.val_at(0)?;
let balance: u64 = rlp.val_at(1)?;
let storage_root_vec: Vec<u8> = rlp.val_at(2)?;
let code_hash_vec: Vec<u8> = rlp.val_at(3)?;
```

The target type is part of the decode operation. If the bytes cannot be decoded
as that type, the call returns an error.

This is why code often has a two-step validation:

```rust
let storage_root_vec: Vec<u8> = rlp.val_at(2)?;

if storage_root_vec.len() != 32 {
    return Err(...);
}
```

RLP can decode the field as bytes, but the account schema still needs to verify
that the bytes are a valid hash length.

## 13. `as_val`

`as_val()` decodes the current RLP value as one Rust value.

This is useful when the top-level value is not a list:

```rust
let value: Vec<u8> = Rlp::new(encoded).as_val().ok()?;
```

This project uses it for storage values because storage values are encoded as a
single byte string, not as a one-item list.

## 14. RLP and MPT Nodes in This Project

The current MPT node encoding follows these shapes.

Branch node:

```text
[
  child_0,
  child_1,
  child_2,
  child_3,
  child_4,
  child_5,
  child_6,
  child_7,
  child_8,
  child_9,
  child_a,
  child_b,
  child_c,
  child_d,
  child_e,
  child_f,
  value
]
```

That is why the encoder uses:

```rust
RlpStream::new_list(17)
```

Leaf node:

```text
[compact_encoded_path_with_leaf_flag, value]
```

Extension node:

```text
[compact_encoded_path_without_leaf_flag, child_ref]
```

Both are lists of two. The compact path flag distinguishes leaf from extension.

## 15. RLP and Hashing

RLP is important because hashes must be computed over deterministic bytes.

For example, an account root depends on:

```text
keccak256(rlp(account))
```

or, for trie nodes:

```text
keccak256(rlp(node))
```

If two logically equal nodes produce different encoded bytes, their hashes will
be different and proofs will break. That is why canonical encoding matters:

- fields must be in a fixed order;
- integers must be minimally encoded;
- missing trie slots must use the same empty representation;
- hashes must be exactly 32 bytes;
- lists must have the expected number of items.

## 16. Common Mistakes

Do not confuse item count with byte length.

```rust
RlpStream::new_list(17)
```

means 17 list items, not 17 bytes. The final RLP prefix is based on the encoded
payload byte length.

Do not use `new_list(1)` when the schema expects a single byte string.

```text
value       != [value]
```

Do not use `append(&encoded_bytes)` when you mean "insert already encoded RLP."
That double-encodes the bytes as data.

Do not assume RLP carries field names. Decode code must know the exact schema.

Do not assume `Vec<u8>` length is valid just because RLP decoded it. RLP can
decode arbitrary bytes, but the caller must validate domain constraints like
hash length or address length.

## 17. Practical Reading Strategy

When reading RLP code, ask these questions in order:

1. Is the top-level value a byte string or a list?
2. If it is a list, how many schema items does it have?
3. For each item, is it bytes, integer-as-bytes, or another list?
4. Are any byte fields constrained by domain rules, such as 20-byte addresses
   or 32-byte hashes?
5. Are any empty byte strings being used as sentinels?
6. Is any field already RLP-encoded and therefore needs raw insertion?

For this repository, most RLP code is straightforward once the schema is clear:

- accounts are 4-item lists;
- headers are 6-item lists;
- transactions are 4-item lists;
- receipts are 3-item lists;
- storage values are single byte strings;
- MPT branches are 17-item lists;
- MPT leaves and extensions are 2-item lists.
