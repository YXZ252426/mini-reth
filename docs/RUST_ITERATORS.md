# Rust Iterators

This note explains Rust iterators from the basic definition to common higher-order
usage patterns. Examples are connected to the MPT code in `src/mpt.rs`.

## 1. What Is an Iterator?

An iterator is a value that can produce a sequence of items one at a time.

At the trait level, the core idea is small:

```rust
pub trait Iterator {
    type Item;

    fn next(&mut self) -> Option<Self::Item>;
}
```

Calling `next` returns:

- `Some(item)` while there are still items;
- `None` when the sequence is exhausted.

Example:

```rust
let values = vec![10, 20, 30];
let mut iter = values.iter();

assert_eq!(iter.next(), Some(&10));
assert_eq!(iter.next(), Some(&20));
assert_eq!(iter.next(), Some(&30));
assert_eq!(iter.next(), None);
```

Most iterator code does not call `next` manually. Instead, it uses iterator
methods such as `map`, `filter`, `zip`, `all`, `collect`, and `count`.

## 2. Iterator, Iterable, and `IntoIterator`

There are two related ideas:

- `Iterator`: the object that actually yields items.
- `IntoIterator`: something that can be converted into an iterator.

For example, a `Vec<T>` is not itself usually used as the iterator. It can create
different iterators depending on how it is accessed.

```rust
let values = vec![1, 2, 3];
let by_ref = values.iter();      // Item = &i32

let mut values = vec![1, 2, 3];
let by_mut = values.iter_mut();  // Item = &mut i32

let values = vec![1, 2, 3];
let by_value = values.into_iter(); // Item = i32
```

The three forms matter:

```rust
let values = vec![1, 2, 3];

for value in values.iter() {
    // value: &i32
}

let mut values = vec![1, 2, 3];

for value in values.iter_mut() {
    // value: &mut i32
    *value += 1;
}

let values = vec![1, 2, 3];

for value in values.into_iter() {
    // value: i32
    // values is moved and cannot be used after this
}
```

In this project, `children.iter_mut().enumerate()` is used when decoding a branch
node:

```rust
for (index, child) in children.iter_mut().enumerate() {
    let data: Vec<u8> = rlp.val_at(index).ok()?;
    // child: &mut Option<NodeRef>
}
```

`iter_mut` is needed because each child slot may be updated.

## 3. Iterators Are Lazy

Iterator adapters usually do not run immediately. They build a pipeline. The
pipeline runs only when a consumer asks for results.

Adapter examples:

- `map`
- `filter`
- `zip`
- `take`
- `take_while`
- `enumerate`
- `skip`

Consumer examples:

- `collect`
- `count`
- `all`
- `any`
- `find`
- `fold`
- `for_each`

Example:

```rust
let values = [1, 2, 3];

let iter = values
    .iter()
    .map(|value| *value * 2);

// Nothing has been collected yet.

let result: Vec<_> = iter.collect();
assert_eq!(result, vec![2, 4, 6]);
```

`map` is lazy. `collect` consumes the iterator.

## 4. Reading This Project's Examples

### `all`

From `compact_encode`:

```rust
assert!(
    path.iter().all(|nibble| *nibble < 16),
    "MPT path contains a non-nibble value"
);
```

Step by step:

```rust
path.iter()
```

creates an iterator over references:

```rust
Item = &Nibble
```

Then:

```rust
.all(|nibble| *nibble < 16)
```

checks whether every nibble is smaller than `16`.

Because `nibble` is `&Nibble`, the code uses `*nibble` to compare the underlying
`u8` value.

Equivalent loop:

```rust
let mut ok = true;

for nibble in path {
    if *nibble >= 16 {
        ok = false;
        break;
    }
}
```

`all` short-circuits. It stops as soon as the predicate returns `false`.

### `map` and `collect`

From `pack_nibbles`:

```rust
nibbles
    .chunks_exact(2)
    .map(|pair| (pair[0] << 4) | pair[1])
    .collect()
```

Step by step:

```rust
nibbles.chunks_exact(2)
```

creates an iterator over fixed-size chunks:

```rust
Item = &[Nibble]
```

Each chunk has length `2`, so `pair[0]` and `pair[1]` are valid.

Then:

```rust
.map(|pair| (pair[0] << 4) | pair[1])
```

turns two nibbles into one byte:

```rust
high nibble: pair[0] << 4
low nibble:  pair[1]
combined:    high | low
```

Finally:

```rust
.collect()
```

consumes the iterator and builds a collection. The function return type is
`Vec<u8>`, so Rust infers that `collect` should produce a `Vec<u8>`.

Equivalent loop:

```rust
let mut bytes = Vec::new();

for pair in nibbles.chunks_exact(2) {
    bytes.push((pair[0] << 4) | pair[1]);
}

bytes
```

### `zip`, `take_while`, and `count`

From `common_prefix_len`:

```rust
fn common_prefix_len(left: &[Nibble], right: &[Nibble]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}
```

This computes the number of equal nibbles at the start of two paths.

Step by step:

```rust
left.iter()
```

creates:

```rust
Item = &Nibble
```

Then:

```rust
.zip(right)
```

pairs each left nibble with the corresponding right nibble.

`right` is a slice. Slices implement `IntoIterator`, so `zip(right)` is roughly
the same as:

```rust
.zip(right.iter())
```

After `zip`, the item type is:

```rust
Item = (&Nibble, &Nibble)
```

Then:

```rust
.take_while(|(left, right)| left == right)
```

keeps taking pairs while the two referenced nibbles are equal.

Finally:

```rust
.count()
```

counts how many matching pairs were taken.

Equivalent loop:

```rust
let mut count = 0;

for (left, right) in left.iter().zip(right.iter()) {
    if left != right {
        break;
    }

    count += 1;
}

count
```

Important detail: `zip` stops when the shorter iterator ends. Therefore this
function naturally handles paths of different lengths.

Example:

```rust
let left = [1, 2, 3, 4];
let right = [1, 2, 9];

assert_eq!(common_prefix_len(&left, &right), 2);
```

## 5. Closures in Iterator Methods

Many iterator methods accept closures.

A closure is an anonymous function:

```rust
|x| x + 1
```

Examples:

```rust
values.iter().map(|value| *value + 1);
values.iter().filter(|value| **value > 10);
values.iter().all(|value| *value < 16);
```

The number of `*` operators depends on the item type and the method signature.

For `iter()` over `Vec<u8>`:

```rust
Item = &u8
```

For `filter`, the predicate receives a reference to the item:

```rust
FnMut(&Item) -> bool
```

So if `Item = &u8`, the closure receives:

```rust
&&u8
```

That is why `filter` often needs one more dereference than `map`:

```rust
let values = vec![1, 2, 3, 4];

let even: Vec<_> = values
    .iter()
    .filter(|value| **value % 2 == 0)
    .collect();
```

Another common style is destructuring the reference:

```rust
let even: Vec<_> = values
    .iter()
    .filter(|&&value| value % 2 == 0)
    .collect();
```

Use whichever style is clearer for the surrounding code.

## 6. Adapter Methods

Iterator adapters transform one iterator into another iterator.

They are lazy.

### `map`

Transforms each item.

```rust
let doubled: Vec<_> = [1, 2, 3]
    .iter()
    .map(|value| *value * 2)
    .collect();

assert_eq!(doubled, vec![2, 4, 6]);
```

Use `map` when every input item becomes one output item.

### `filter`

Keeps only items that satisfy a predicate.

```rust
let values = [1, 2, 3, 4];

let even: Vec<_> = values
    .iter()
    .filter(|value| **value % 2 == 0)
    .collect();

assert_eq!(even, vec![&2, &4]);
```

Use `filter` when some items should be skipped.

### `filter_map`

Maps and filters at the same time.

```rust
let inputs = ["1", "two", "3"];

let numbers: Vec<i32> = inputs
    .iter()
    .filter_map(|text| text.parse::<i32>().ok())
    .collect();

assert_eq!(numbers, vec![1, 3]);
```

Use `filter_map` when conversion may fail and failed conversions should be
ignored.

### `flat_map`

Maps each item into an iterator and flattens the result.

```rust
let nested = vec![vec![1, 2], vec![3, 4]];

let flat: Vec<_> = nested
    .iter()
    .flat_map(|inner| inner.iter())
    .collect();

assert_eq!(flat, vec![&1, &2, &3, &4]);
```

Use `flat_map` when one input item can produce many output items.

### `enumerate`

Adds an index.

```rust
for (index, value) in ["a", "b", "c"].iter().enumerate() {
    println!("{index}: {value}");
}
```

This project uses it when filling branch children:

```rust
for (index, child) in children.iter_mut().enumerate() {
    let data: Vec<u8> = rlp.val_at(index).ok()?;
    *child = Some(hash);
}
```

### `zip`

Combines two iterators item by item.

```rust
let names = ["alice", "bob"];
let scores = [10, 20];

let pairs: Vec<_> = names.iter().zip(scores.iter()).collect();

assert_eq!(pairs, vec![(&"alice", &10), (&"bob", &20)]);
```

`zip` stops when either side ends.

### `take` and `skip`

`take(n)` keeps the first `n` items.

```rust
let first_two: Vec<_> = [1, 2, 3]
    .iter()
    .take(2)
    .collect();

assert_eq!(first_two, vec![&1, &2]);
```

`skip(n)` skips the first `n` items.

```rust
let rest: Vec<_> = [1, 2, 3]
    .iter()
    .skip(1)
    .collect();

assert_eq!(rest, vec![&2, &3]);
```

### `take_while` and `skip_while`

`take_while` keeps items until the predicate becomes false.

```rust
let prefix: Vec<_> = [1, 2, 3, 1]
    .iter()
    .take_while(|value| **value < 3)
    .collect();

assert_eq!(prefix, vec![&1, &2]);
```

`skip_while` skips items until the predicate becomes false.

```rust
let rest: Vec<_> = [1, 2, 3, 1]
    .iter()
    .skip_while(|value| **value < 3)
    .collect();

assert_eq!(rest, vec![&3, &1]);
```

### `chain`

Concatenates two iterators.

```rust
let combined: Vec<_> = [1, 2]
    .iter()
    .chain([3, 4].iter())
    .collect();

assert_eq!(combined, vec![&1, &2, &3, &4]);
```

### `cloned` and `copied`

`iter()` yields references. Sometimes the result should own values.

For `Copy` types such as `u8`, use `copied`:

```rust
let owned: Vec<u8> = [1u8, 2, 3]
    .iter()
    .copied()
    .collect();

assert_eq!(owned, vec![1, 2, 3]);
```

For `Clone` types such as `String`, use `cloned`:

```rust
let values = vec![String::from("a"), String::from("b")];

let owned: Vec<String> = values
    .iter()
    .cloned()
    .collect();
```

## 7. Consumer Methods

Consumers run the iterator pipeline and produce a final result.

### `collect`

Builds a collection.

```rust
let values: Vec<_> = [1, 2, 3]
    .iter()
    .map(|value| *value * 2)
    .collect();
```

Sometimes Rust needs a type hint:

```rust
let values = [1, 2, 3]
    .iter()
    .map(|value| *value * 2)
    .collect::<Vec<_>>();
```

### `count`

Counts items.

```rust
let count = [1, 2, 3].iter().count();

assert_eq!(count, 3);
```

In `common_prefix_len`, `count` counts the number of matching prefix pairs.

### `all` and `any`

`all` returns true only if every item passes.

```rust
let all_nibbles = path.iter().all(|nibble| *nibble < 16);
```

`any` returns true if at least one item passes.

```rust
let has_zero = path.iter().any(|nibble| *nibble == 0);
```

Both short-circuit.

### `find`

Returns the first matching item.

```rust
let values = [1, 3, 4, 6];

let first_even = values
    .iter()
    .find(|value| **value % 2 == 0);

assert_eq!(first_even, Some(&4));
```

### `position`

Returns the index of the first matching item.

```rust
let values = [1, 3, 4, 6];

let index = values
    .iter()
    .position(|value| *value % 2 == 0);

assert_eq!(index, Some(2));
```

### `fold`

Accumulates a result.

```rust
let sum = [1, 2, 3]
    .iter()
    .fold(0, |acc, value| acc + *value);

assert_eq!(sum, 6);
```

Equivalent loop:

```rust
let mut acc = 0;

for value in [1, 2, 3].iter() {
    acc += *value;
}
```

Use `fold` when each item updates an accumulated state.

### `try_fold`

Like `fold`, but can stop early with `Result` or `Option`.

```rust
let parsed = ["1", "2", "bad", "4"]
    .iter()
    .try_fold(Vec::new(), |mut values, text| {
        let value = text.parse::<i32>().ok()?;
        values.push(value);
        Some(values)
    });

assert_eq!(parsed, None);
```

Use `try_fold` when the pipeline can fail.

## 8. Ownership Patterns

The most common iterator confusion in Rust is ownership.

### Borrowing With `iter`

```rust
let values = vec![String::from("a"), String::from("b")];

for value in values.iter() {
    // value: &String
}

// values is still usable
assert_eq!(values.len(), 2);
```

### Mutating With `iter_mut`

```rust
let mut values = vec![1, 2, 3];

for value in values.iter_mut() {
    *value += 1;
}

assert_eq!(values, vec![2, 3, 4]);
```

### Moving With `into_iter`

```rust
let values = vec![String::from("a"), String::from("b")];

for value in values.into_iter() {
    // value: String
}

// values is no longer usable here
```

For arrays, modern Rust also supports by-value array iteration:

```rust
let values = [1, 2, 3];

for value in values.into_iter() {
    // value: i32
}
```

## 9. Returning Iterators

Iterator types are often long. Use `impl Iterator` when returning one from a
function.

```rust
fn even_values(values: &[i32]) -> impl Iterator<Item = &i32> {
    values.iter().filter(|value| **value % 2 == 0)
}
```

If the iterator captures references, lifetimes may need to be explicit in more
complex cases:

```rust
fn even_values<'a>(values: &'a [i32]) -> impl Iterator<Item = &'a i32> {
    values.iter().filter(|value| **value % 2 == 0)
}
```

## 10. Implementing a Custom Iterator

A custom iterator stores state and implements `next`.

```rust
struct Counter {
    current: usize,
    end: usize,
}

impl Iterator for Counter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }

        let value = self.current;
        self.current += 1;
        Some(value)
    }
}

let values: Vec<_> = Counter { current: 0, end: 3 }.collect();

assert_eq!(values, vec![0, 1, 2]);
```

Usually you do not need custom iterators. Standard iterator adapters cover most
cases.

## 11. How to Choose Between Loops and Iterators

Use iterator chains when the operation is a clear data transformation:

```rust
let bytes: Vec<u8> = nibbles
    .chunks_exact(2)
    .map(|pair| (pair[0] << 4) | pair[1])
    .collect();
```

Use a loop when there is complex branching, mutation of several variables, or
when the iterator version would be harder to read:

```rust
let mut nibbles = Vec::with_capacity(bytes.len() * 2);

for byte in bytes {
    nibbles.push(byte >> 4);
    nibbles.push(byte & 0x0f);
}
```

Both styles are idiomatic. The better choice is the one that makes ownership,
control flow, and data transformation obvious.

## 12. Common Mistakes

### Forgetting That Adapters Are Lazy

This does not do anything visible:

```rust
values.iter().map(|value| *value + 1);
```

Use a consumer:

```rust
let new_values: Vec<_> = values
    .iter()
    .map(|value| *value + 1)
    .collect();
```

### Confusing `&T` and `T`

`iter()` gives references:

```rust
path.iter().all(|nibble| *nibble < 16)
```

If you want copied `u8` values:

```rust
path.iter().copied().all(|nibble| nibble < 16)
```

For this project, the second form can sometimes be easier to read because
`Nibble` is just `u8`.

### Overusing Iterator Chains

Long chains can hide important control flow. Prefer splitting the chain:

```rust
let matching_prefix = left
    .iter()
    .zip(right)
    .take_while(|(left, right)| left == right);

matching_prefix.count()
```

or use a loop if it is clearer.

## 13. Mental Model

Read iterator chains from left to right:

```rust
left.iter()
    .zip(right)
    .take_while(|(left, right)| left == right)
    .count()
```

means:

1. iterate over the left path;
2. pair each left nibble with a right nibble;
3. keep pairs while they are equal;
4. count how many pairs survived.

For this MPT implementation, iterators are mostly used for:

- validating every nibble;
- packing pairs of nibbles into bytes;
- finding common path prefixes;
- walking branch child arrays with indexes;
- checking test invariants over arrays.

The key skill is to track the item type after each step. Once the item type is
clear, the closure syntax becomes much easier to reason about.
