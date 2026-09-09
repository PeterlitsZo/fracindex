# Fracindex

A fast, space-efficient fractional index implementation.

It can help you assign stable, sortable positions to ordered values (e.g. a
list of TODO items, and you want to keep their order).

Read the [Reference](https://docs.rs/fracindex/latest/fracindex/) for more
information.

## Getting Started

To use `fracindex`, add it to your `Cargo.toml` dependencies:

```toml
fracindex = "0.3.1"
```

You can create a new `Fracindex` and calculate a new `Fracindex` using existing
`Fracindex` values.

```rust
use fracindex::Fracindex;

let first = Fracindex::default();
let second = Fracindex::builder().after(&first).build().unwrap();
let middle = Fracindex::builder().between(&first, &second).build().unwrap();

println!("{first:?} {middle:?} {second:?}");
// -> Fracindex(7fffffff) Fracindex(8000007f) Fracindex(800000ff)

assert!(first < middle);
assert!(middle < second);
```

We have many methods:

- `builder().after(...).build()`. Create a `Fracindex` that is after a given
  `Fracindex`.
- `builder().before(...).build()`. Create a `Fracindex` that is before a given
  `Fracindex`.
- `builder().between(..., ...).build()`. Create a `Fracindex` that is between
  two given `Fracindex` values.
- `builder().after(...).batch_build(count)`. Create a batch of `Fracindex`
  values that are after a given `Fracindex`.
- `builder().before(...).batch_build(count)`. Create a batch of `Fracindex`
  values that are before a given `Fracindex`.
- `builder().between(..., ...).batch_build(count)`. Create a batch of
  `Fracindex` values that are between two given `Fracindex` values.

Use `.space_policy(SpacePolicy::Random)` on a builder to allocate midpoint-like
space near open bounds. The default is `SpacePolicy::Sequential`, which is
suitable for repeated appends and prepends.

The `Fracindex` can be turned into bytes and back again using
`Fracindex::from_bytes` and `Fracindex::to_bytes`:

```rust
let bytes = first.to_bytes();
let restored = Fracindex::from_bytes(bytes).unwrap();
assert_eq!(first, restored);
```

It is very helpful if you want to put it in a database or serialize it. You can
use `bytes_len` to get the number of bytes required to store the `Fracindex`
without allocating any memory.

Sometimes repeated insertions into the same small range make indexes longer.
When that happens, you can rebalance a sorted slice:

```rust
use fracindex::{Fracindex, RebalancePolicy};

let first = Fracindex::default();
let last = Fracindex::builder().after(&first).build().unwrap();
let mut indexes = vec![first.clone(), last.clone()];

for _ in 0..10 {
    let next = Fracindex::builder()
        .between(&indexes[0], &indexes[1])
        .build()
        .unwrap();
    indexes.insert(1, next);
}
println!("{indexes:#?}");
// => [
//        Fracindex(7fffffff),
//        Fracindex(7fffffff3fffffff),
//        Fracindex(7fffffff7fffffff),
//        Fracindex(80),
//        Fracindex(80000001),
//        Fracindex(80000003),
//        Fracindex(80000007),
//        Fracindex(8000000f),
//        Fracindex(8000001f),
//        Fracindex(8000003f),
//        Fracindex(8000007f),
//        Fracindex(800000ff),
//    ]

let rebalanced = Fracindex::rebalance(
    &indexes,
    RebalancePolicy::PreserveBoth
).unwrap();

assert_eq!(rebalanced.first(), Some(&first));
assert_eq!(rebalanced.last(), Some(&last));
assert!(rebalanced.windows(2).all(|pair| pair[0] < pair[1]));

println!("{rebalanced:#?}");
// => [
//        Fracindex(7fffffff),
//        Fracindex(8000000f),
//        Fracindex(8000001f),
//        Fracindex(8000003f),
//        Fracindex(8000004f),
//        Fracindex(8000005f),
//        Fracindex(8000007f),
//        Fracindex(8000008f),
//        Fracindex(8000009f),
//        Fracindex(800000bf),
//        Fracindex(800000df),
//        Fracindex(800000ff),
//    ]
```

`RebalancePolicy::PreserveBoth` keeps the first and last index unchanged.
`PreserveFirst` keeps only the first one, `PreserveLast` keeps only the last
one, and `ReplaceAll` creates a new sequence for the whole slice. The input
slice must already be sorted in ascending order.

## Benchmarks

This repository contains Criterion benchmarks comparing `fracindex` with the
`fractional_index` crate across single operations and larger workloads:

- Creating the default index.
- Creating indexes before, after, and between existing indexes.
- Append, prepend, dense-between, and random-insert workloads.

Run them with:

```sh
cargo bench --bench comparison
```

On my macOS, the mean point estimates from Criterion are:

| Benchmark                | `fracindex` | `fractional_index` | Speedup |
| ------------------------ | ----------: | -----------------: | ------: |
| default                  | 1.19 ns     | 8.78 ns            | 7.4x    |
| builder_before/short     | 4.22 ns     | 22.85 ns           | 5.4x    |
| builder_before/16_bytes  | 5.88 ns     | 37.56 ns           | 6.4x    |
| builder_before/64_bytes  | 18.08 ns    | 54.32 ns           | 3.0x    |
| builder_after/short      | 4.08 ns     | 23.24 ns           | 5.7x    |
| builder_after/16_bytes   | 5.45 ns     | 37.36 ns           | 6.9x    |
| builder_after/64_bytes   | 17.88 ns    | 54.43 ns           | 3.0x    |
| builder_between/short    | 5.76 ns     | 22.05 ns           | 3.8x    |
| builder_between/16_bytes | 18.68 ns    | 29.65 ns           | 1.6x    |
| builder_between/64_bytes | 18.81 ns    | 47.58 ns           | 2.5x    |
| append/100000            | 769.44 us   | 19.15 ms           | 24.9x   |
| prepend/100000           | 757.99 us   | 18.12 ms           | 23.9x   |
| dense_between/100000     | 8.68 ms     | 17.53 ms           | 2.0x    |
| random_insert/100000     | 1.17 ms     | 6.39 ms            | 5.5x    |
