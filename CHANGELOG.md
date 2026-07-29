# CHANGELOG

Please follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Add the initial fractional indexing implementation and public API.
- Add random and sequential policies for generating indexes before or after
  existing values.

### Changed

- Require a `FracindexPolicy` when calling `Fracindex::new_before` or
  `Fracindex::new_after`.
