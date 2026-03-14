# Contributing to graydr

Thank you for considering a contribution to graydr. This document covers
what you need to know before submitting.

## Contributor License Agreement

Before your contribution can be accepted, you must agree to the
[Contributor License Agreement](CLA.md). By opening a pull request you
confirm that you have read the CLA and agree to its terms.

If you are contributing on behalf of an employer, or if your employer may
have rights to the work, please contact legal@momidala.com before submitting.

## Licensing

- `graydr` (compiler) is licensed under AGPL 3.0 with an output exception.
  See [LICENSE](LICENSE).
- `graydr-registry` is licensed under AGPL 3.0. See
  [graydr-registry/LICENSE](graydr-registry/LICENSE).

Your contributions to each crate will be licensed under the same terms.

## How to Contribute

1. Fork the repository and create a branch from `main`.
2. Make your changes. Add or update tests as appropriate.
3. Ensure `cargo test` passes in full (`cargo test --workspace`).
4. Ensure `cargo clippy --workspace` passes without warnings.
5. Open a pull request with a clear description of what changes and why.

## What We're Looking For

- Bug fixes with a test that demonstrates the fix
- Performance improvements with benchmarks where applicable
- Documentation improvements
- New reference modules following the conventions in
  `docs/module-style-guide.md`

For significant new features or changes to the language spec, please open
an issue for discussion before investing time in an implementation.

## Code Style

- Follow standard Rust idioms (`cargo fmt` before submitting)
- Error messages should be lowercase, no trailing punctuation
- Span information should be preserved through any new AST nodes
- New CLI flags should have help text consistent with existing flags

## Questions

Open an issue or start a discussion in the repository.
