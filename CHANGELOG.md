# Changelog

## [0.0.2] - 2020-10-23

### Added
- `autobox-cli` can now *infer* side effects on functions that do not have a `declare` macro on them. You can see the example in the readme.
- `autobox-cli` can now trace variables that are not static, including variables declared by moving a value into it, by function call, 


### Changed
- Moved from a handwritten parser to one based on `nom`. It's much more resilient to whitespace, optional values, etc.

## [0.0.1] - 2020-10-16

### First release
- `autobox-cli` and `declare` macro skeleton implemented
- No inference outside of `entrypoint` but can understand `declare` macros
