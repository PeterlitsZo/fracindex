# CHANGELOG

Please follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased

### Added

- Add key-value context to `FracindexError`.

### Changed

- [BREAKING] The `FracindexError` struct is now only having private fields.
- [BREAKING] Change `Fracindex::new_between` to return the existing index when
  both bounds are equal instead of failing.

## v0.3.1 (2026-08-17)

### Fixed

- Fix the bad batch creation methods for `Fracindex`. The original name
  `new_before_batch` is changed to `batch_new_before`. The original name
  `new_after_batch` is changed to `batch_new_after`.

## v0.3.0 (2026-08-17)

### Added

- Support `batch_new_before`, `batch_new_after` and `batch_new_between` methods
  for `Fracindex`.

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
