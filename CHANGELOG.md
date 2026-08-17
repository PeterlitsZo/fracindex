# CHANGELOG

Please follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## v0.2.0 (2026-08-17)

### Added

- Add `Fracindex::from_hex` for decoding hexadecimal index representations.
- Add index rebalancing with configurable endpoint and allocation policies.
- Add `Fracindex::bytes_len` for obtaining the encoded length without
  allocating.
- Add structured error types for fallible fractional-index operations.

### Changed

- \[BREAKING] Change `Fracindex::from_bytes`, `Fracindex::from_hex`, and
  `Fracindex::new_between` to return `FracindexResult` with error details.

## v0.1.0 (2026-07-30)

### Added

- Add the initial fractional indexing implementation and public API.
