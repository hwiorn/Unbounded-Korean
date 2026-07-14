# Unbounded Korean Umbrella Design

## Goal

Expose the project as an umbrella package named `unbounded-korean` while keeping the focused component crates and Python modules available.

## Rust Surface

The root package is named `unbounded-korean` and its library crate is `unbounded_korean`. It re-exports:

- `g2pk` public API
- `korean_phonemizer`
- `hangulize_rs`

This lets Rust users depend on one crate for the full toolkit while preserving direct sub-crate usage for smaller builds.

## Python Surface

Add a PyO3 extension crate under `crates/unbounded-korean-py`.

- Python distribution name: `unbounded-korean`
- Python import name: `unbounded_korean`
- Exposed classes: `G2p`, `KoreanPhonemizer`, `Hangulizer`
- Exposed functions: `g2p`, `phonemize_ko`, `hangulize`, `list_langs`

The existing separate Python modules remain available and are not merged into a single extension crate internally.

## Compatibility

Existing Rust sub-crates and PyO3 crates remain in the workspace. The umbrella package is additive except for renaming the root package from the temporary project name to the distribution name.

## Verification

Rust integration tests verify re-exports. PyO3 verification uses `cargo check -p unbounded-korean-py --features extension-module`, and Python smoke tests use `maturin develop` for the umbrella crate when available.
