# Separated PyO3 Modules and Hangulize-RS Design

## Goal

Keep each runtime package aligned with its own domain: Korean G2P, Korean IPA phonemization, and foreign-script-to-Hangul transliteration. Rust core crates remain reusable without Python, and each PyO3 extension exposes only the API for its own crate.

## Scope

- `g2pk-py` remains the Python extension for `g2pk`.
- `korean_phonemizer-py` exposes `korean_phonemizer` as a separate Python extension.
- `hangulize-rs` is a Rust port of the HSL-driven transliteration engine.
- `hangulize-py` exposes `hangulize-rs` as a separate Python extension.

No code or documentation should add general source-provenance notes. Rule comments may describe behavior or compatibility fixes when they materially help maintenance.

## Architecture

`hangulize-rs` uses a pipeline: load embedded HSL spec, parse sections, compile HRE rules, normalize input by script, partition meaningful text, rewrite, transcribe, compose Hangul syllables, and localize punctuation. HSL specs are embedded in the crate and cached per language.

`korean_phonemizer-py` and `hangulize-py` are thin adapters. They map Python arguments to Rust options and translate Rust errors into Python exceptions.

## Public APIs

Rust:

- `hangulize_rs::list_langs()`
- `hangulize_rs::hangulize(lang, word)`
- `hangulize_rs::load_spec(lang)`
- `hangulize_rs::Hangulizer`

Python:

- `korean_phonemizer.phonemize_ko(text, ...)`
- `korean_phonemizer.KoreanPhonemizer(...)`
- `hangulize_rs.hangulize(lang, word)`
- `hangulize_rs.list_langs()`
- `hangulize_rs.Hangulizer(lang)`

## Testing

Tests cover crate boundaries, PyO3 build checks, representative HSL examples, bundled language listing, unknown-language errors, punctuation preservation, and selected language examples from embedded spec tests.
