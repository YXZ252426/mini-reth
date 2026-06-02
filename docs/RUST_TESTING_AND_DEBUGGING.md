# Rust Testing and Debugging

This note explains how to write tests and debug failures in Rust projects. The
examples are general Rust examples, with a few small examples from the MPT code.

## 1. What Testing Gives You

Rust's compiler catches many classes of bugs:

- invalid ownership;
- use-after-free;
- data races in safe code;
- many type mismatches;
- missing handling for some enum patterns.

But the compiler does not know whether the program behavior is correct.

Tests are still needed for questions like:

- does this function return the right value?
- does insertion preserve old entries?
- does decoding reject malformed input?
- does a boundary case behave correctly?
- does a previous bug stay fixed?

For data structure projects, tests are especially important because a small local
mistake can produce a valid Rust program with the wrong logical shape.

## 2. Basic Test Syntax

Rust tests are ordinary functions marked with `#[test]`.

```rust
#[test]
fn adds_two_numbers() {
    assert_eq!(2 + 2, 4);
}
```

Common assertions:

```rust
assert!(condition);
assert_eq!(left, right);
assert_ne!(left, right);
```

Examples:

```rust
#[test]
fn option_contains_value() {
    let value = Some(10);

    assert_eq!(value, Some(10));
}

#[test]
fn vector_has_expected_items() {
    let values = vec![1, 2, 3];

    assert_eq!(values, vec![1, 2, 3]);
}
```

Use `assert_eq!` when the expected value matters. Its failure output shows both
sides:

```text
assertion `left == right` failed
  left: ...
 right: ...
```

## 3. Unit Tests and Test Modules

Unit tests are usually placed near the code they test.

```rust
fn double(value: i32) -> i32 {
    value * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn double_multiplies_by_two() {
        assert_eq!(double(3), 6);
    }
}
```

`#[cfg(test)]` means the test module is compiled only when running tests.

`use super::*;` imports items from the parent module, which lets tests access
private functions in the same file.

This style is useful for testing internal helpers such as encoders, decoders, or
small path-manipulation functions.

## 4. Integration Tests

Integration tests live in the top-level `tests/` directory.

Example layout:

```text
src/
  lib.rs
  parser.rs
tests/
  parser_behavior.rs
```

Integration tests use the crate as an external user would:

```rust
use my_crate::parse;

#[test]
fn parses_valid_input() {
    assert!(parse("123").is_ok());
}
```

Use integration tests for public API behavior. Use unit tests for private helper
logic and narrow edge cases.

## 5. Running Tests

Run all tests:

```sh
cargo test
```

Run tests whose names contain a substring:

```sh
cargo test insert
```

Run one exact test path:

```sh
cargo test mpt::tests::mpt_insert_empty_key_is_readable
```

Run only library tests:

```sh
cargo test --lib
```

Show `println!` and `dbg!` output:

```sh
cargo test test_name -- --nocapture
```

Show a backtrace on panic:

```sh
RUST_BACKTRACE=1 cargo test test_name
```

## 6. Naming Tests

Good test names describe behavior.

Prefer:

```rust
#[test]
fn insert_same_key_overwrites_value() {}

#[test]
fn decode_rejects_invalid_hash_length() {}

#[test]
fn empty_input_returns_none() {}
```

Avoid vague names:

```rust
#[test]
fn test_insert() {}

#[test]
fn works() {}
```

A useful pattern is:

```text
function_or_feature_condition_expected_behavior
```

Examples:

```text
compact_decode_rejects_invalid_inputs
insert_long_key_after_short_key_preserves_short_value
parser_empty_string_returns_error
```

## 7. Choosing Test Cases

A good test set usually includes more than the happy path.

Common categories:

- happy path;
- empty input;
- invalid input;
- missing value;
- duplicate insert or overwrite;
- prefix or boundary conflict;
- round trip encode/decode;
- malformed serialized data;
- regression case for a previous bug.

For example, if testing a map-like data structure:

```rust
#[test]
fn get_missing_key_returns_none() {}

#[test]
fn insert_then_get_returns_value() {}

#[test]
fn insert_same_key_overwrites_old_value() {}

#[test]
fn insert_two_keys_preserves_both_values() {}
```

For a parser or decoder:

```rust
#[test]
fn decode_valid_input_returns_value() {}

#[test]
fn decode_empty_input_returns_none() {}

#[test]
fn decode_invalid_tag_returns_none() {}

#[test]
fn encode_then_decode_round_trips() {}
```

## 8. Testing `Option` and `Result`

Testing `Option`:

```rust
assert_eq!(lookup("alice"), Some(10));
assert_eq!(lookup("missing"), None);
```

Testing `Result`:

```rust
assert_eq!(parse_number("123"), Ok(123));
assert!(parse_number("abc").is_err());
```

For more detailed `Result` checks:

```rust
let err = parse_number("abc").unwrap_err();
assert_eq!(err.kind(), ErrorKind::InvalidDigit);
```

Use `unwrap` in tests when the test should fail immediately if a value is absent:

```rust
let user = find_user("alice").unwrap();
assert_eq!(user.name, "alice");
```

This is acceptable in tests because a panic is a clear test failure.

## 9. Testing Custom Types

For `assert_eq!` to work, the type needs `PartialEq`. For useful failure output,
it also needs `Debug`.

```rust
#[derive(Debug, PartialEq, Eq)]
struct Account {
    nonce: u64,
    balance: u128,
}

#[test]
fn account_starts_empty() {
    assert_eq!(
        Account {
            nonce: 0,
            balance: 0,
        },
        Account {
            nonce: 0,
            balance: 0,
        }
    );
}
```

For enums:

```rust
#[derive(Debug, PartialEq, Eq)]
enum Node {
    Empty,
    Leaf(Vec<u8>),
}

#[test]
fn creates_leaf() {
    assert_eq!(Node::Leaf(vec![1, 2]), Node::Leaf(vec![1, 2]));
}
```

## 10. Round-Trip Tests

Round-trip tests are useful for encoders and decoders.

The shape is:

```rust
#[test]
fn encoding_round_trips() {
    let original = Value::new("hello");

    let encoded = original.encode();
    let decoded = Value::decode(&encoded).unwrap();

    assert_eq!(decoded, original);
}
```

Round-trip tests catch mismatches between serialization and deserialization.

They do not prove the encoded format is externally correct. For that, add tests
with known expected bytes.

## 11. Regression Tests

A regression test captures a bug that already happened.

Example:

```rust
#[test]
fn empty_input_does_not_panic() {
    assert_eq!(parse(""), None);
}
```

The point is not only to verify today's implementation. The point is to prevent a
future change from reintroducing the same bug.

When fixing a bug, first write or identify the failing test. Then fix the code.

## 12. Reading Failure Output

Example failure:

```text
thread 'tests::insert_empty_key_is_readable' panicked at src/lib.rs:42:9:
assertion `left == right` failed
  left: Some([111, 116, 104, 101, 114])
 right: Some([101, 109, 112, 116, 121])
```

Read it in this order:

1. Test name: which behavior failed?
2. File and line: which assertion failed?
3. Left value: what the code actually returned.
4. Right value: what the test expected.

For `Vec<u8>`, byte arrays often represent ASCII text:

```text
[111, 116, 104, 101, 114] = "other"
[101, 109, 112, 116, 121] = "empty"
```

That means the program returned `"other"` where `"empty"` was expected.

## 13. Debugging Workflow

A reliable debugging workflow:

1. Reproduce the failure.
2. Read the failing test.
3. Identify the smallest input that fails.
4. Find the public function called by the test.
5. Follow the data through internal helpers.
6. Locate the first place where state becomes wrong.
7. Explain the bug before editing.
8. Make the smallest fix.
9. Run the failing test again.
10. Run related tests.
11. Run the full test suite.

The most important step is finding the first wrong state. The final wrong return
value is often far away from the real bug.

## 14. Isolating the Failure

When a complex test fails, reduce it mentally or with a smaller test.

Example:

```rust
map.insert("abc", 1);
map.insert("ab", 2);

assert_eq!(map.get("ab"), Some(2));
assert_eq!(map.get("abc"), Some(1));
```

This is easier to reason about than a long scenario with many inserts.

Ask:

- What was the structure after the first operation?
- What should the second operation change?
- What must remain unchanged?
- Which assertion proves the new behavior?
- Which assertion proves old behavior was preserved?

## 15. Print Debugging

Rust gives you a few simple tools.

### `println!`

```rust
println!("path = {:?}", path);
```

To see this output during tests:

```sh
cargo test test_name -- --nocapture
```

### `dbg!`

`dbg!` prints the file, line, expression, and value. It also returns the value.

```rust
let shared_len = dbg!(common_prefix_len(left, right));
```

For temporary debugging:

```rust
dbg!(&node);
dbg!(&remaining_path);
```

Remove `dbg!` before committing unless the output is intentionally part of the
program.

### `eprintln!`

`eprintln!` writes to stderr:

```rust
eprintln!("decoded = {:?}", decoded);
```

This can be useful when stdout is already used by the program.

## 16. Backtraces

Use a backtrace when the panic location is not enough.

```sh
RUST_BACKTRACE=1 cargo test test_name
```

Use a fuller backtrace if needed:

```sh
RUST_BACKTRACE=full cargo test test_name
```

Backtraces are most useful for unexpected panics, not ordinary `assert_eq!`
failures where the failing assertion already points to the behavior.

## 17. Debugging `Option` Pipelines

Rust code often uses `?` with `Option`:

```rust
let value = maybe_value?;
```

If the function unexpectedly returns `None`, one of the earlier `Option`
expressions returned `None`.

Debug it by expanding the pipeline:

```rust
let value = match maybe_value {
    Some(value) => value,
    None => {
        dbg!("missing maybe_value");
        return None;
    }
};
```

This is temporary debugging code. It helps identify which step failed.

## 18. Debugging `Result` Pipelines

For `Result`, prefer preserving error context.

Basic version:

```rust
let value = parse(input)?;
```

More debuggable version:

```rust
let value = parse(input)
    .map_err(|err| format!("failed to parse {input:?}: {err}"))?;
```

In application code, structured error types are better than strings. In small
learning projects, readable error messages are already a major improvement over
silent `None`.

## 19. Testing Panics

If a function is expected to panic:

```rust
#[test]
#[should_panic]
fn rejects_invalid_input() {
    parse_strict("");
}
```

You can also match part of the panic message:

```rust
#[test]
#[should_panic(expected = "input must not be empty")]
fn rejects_empty_input() {
    parse_strict("");
}
```

Use panic tests sparingly. For library-style code, returning `Result` is often
better than panicking.

## 20. Property-Style Thinking

Even without a property-testing library, you can think in properties.

Examples:

- encoding then decoding should return the original value;
- inserting a key should make that key readable;
- inserting a different key should not remove the first key;
- sorting should produce values in nondecreasing order;
- parsing invalid input should not panic.

You can encode these as normal tests with several inputs:

```rust
#[test]
fn encode_decode_round_trips_for_multiple_values() {
    for value in ["", "a", "hello", "with spaces"] {
        let encoded = encode(value);
        let decoded = decode(&encoded).unwrap();

        assert_eq!(decoded, value);
    }
}
```

## 21. Avoiding Over-Specific Tests

Tests should protect important behavior without making refactors painful.

Good:

```rust
assert_eq!(cache.get("a"), Some(1));
```

Risky if not required:

```rust
assert_eq!(cache.internal_buckets.len(), 16);
```

Test internal structure when structure is part of the behavior. For example,
encoding format and trie shape may be meaningful in a Merkle trie. But for most
application code, public behavior is the better test target.

## 22. A Small MPT Example

This kind of test checks a prefix-key edge case:

```rust
#[test]
fn insert_long_key_after_short_key_preserves_branch_value() {
    let mut trie = MptTrie::new();

    trie.insert(b"\x12", b"short".to_vec());
    trie.insert(b"\x12\x34", b"long".to_vec());

    assert_eq!(trie.get(b"\x12"), Some(b"short".to_vec()));
    assert_eq!(trie.get(b"\x12\x34"), Some(b"long".to_vec()));
}
```

The key behavior is not specific to the test name. It is this property:

```text
If one key is a prefix of another key, both values must remain readable.
```

When this fails, inspect the code that handles path splitting. In an MPT, a key
that ends at a branch should store its value in the branch value slot, while the
longer key continues through a child.

## 23. Interpreting Multiple Failures

Multiple failing tests often have one root cause.

Example symptoms:

```text
empty key returns another key's value
short key returns None after inserting long key
short key returns long key's value
```

These look different, but they point to the same class of bug:

```text
The implementation is mishandling the case where a path ends exactly at a branch.
```

When several tests fail:

1. Group them by behavior.
2. Find the shared operation.
3. Ignore unrelated passing details.
4. Look for the first helper used by all failing cases.

Do not patch each symptom independently.

## 24. Command Checklist

Run all tests:

```sh
cargo test
```

Run a single test:

```sh
cargo test test_name
```

Run with debug output:

```sh
cargo test test_name -- --nocapture
```

Run with backtrace:

```sh
RUST_BACKTRACE=1 cargo test test_name
```

Run only library tests:

```sh
cargo test --lib
```

Check without running:

```sh
cargo check
```

Format code:

```sh
cargo fmt
```

Run lints if the project uses Clippy:

```sh
cargo clippy
```

## 25. Personal Debug Checklist

Use this checklist when a test fails:

- What behavior does the test name describe?
- What exact input does the test use?
- Which assertion failed?
- What is the actual value?
- What is the expected value?
- Is this a read bug, write bug, encode/decode bug, or test bug?
- What is the first point where state becomes wrong?
- Can the failure be reproduced with fewer steps?
- Are other failing tests the same root cause?
- After the fix, which test proves the bug is gone?
- Which related tests should also run?

The goal is not just to make the test green. The goal is to understand why the
test was red.
