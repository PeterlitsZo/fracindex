# Fracindex

A fast, space-efficient fractional index implementation.

Example:

```rust
use fracindex::Fracindex;

let first = Fracindex::default();
let second = Fracindex::new_after(&first);
let middle = Fracindex::new_between(&first, &second).unwrap();

println!("{first:?} {middle:?} {second:?}");
// -> Fracindex(7fffffff) Fracindex(8000007f) Fracindex(800000ff)

assert!(first < middle);
assert!(middle < second);
```

The `Fracindex` can be turned into bytes and back again using
`Fracindex::from_bytes` and `Fracindex::to_bytes`:

```rust
let bytes = first.to_bytes();
let restored = Fracindex::from_bytes(bytes).unwrap();
assert_eq!(first, restored);
```
