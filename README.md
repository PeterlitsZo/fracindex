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
