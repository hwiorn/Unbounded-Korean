# g2pk-rs Complete Port Design

## Goal

Build a Rust workspace that provides Korean grapheme-to-pronunciation conversion as a native crate, a Python extension module, and a Korean IPA phonemizer crate. The Rust API is the primary interface; Python bindings mirror the main runtime behavior for existing Python consumers.

## Scope

The first complete port includes:

- `g2pk`: core Rust library for text normalization, pronunciation conversion, embedded resources, optional custom resources, and morphology-backed annotation.
- `korean_phonemizer`: Rust library that uses the core converter and emits Korean IPA with style and compatibility options.
- `g2pk-py`: PyO3 extension module named `g2pk_rs`.
- Workspace tests covering core conversion, custom resources, phonemizer behavior, and Python-facing API shape where build tooling is available.

The implementation must not add general origin notes in comments or docs. Comments are reserved for non-obvious behavior, compatibility deviations, or local bug fixes.

## Architecture

`g2pk` owns Hangul decomposition/composition, resource loading, numeric normalization, English loanword conversion hooks, morphology annotation, pronunciation rule execution, and options. Built-in resources are compiled with `include_str!`; callers may provide a custom rule table and idiom table through configuration. Custom resources take precedence over embedded resources for that `G2p` instance.

Morphological analysis is abstracted behind a small trait so the converter is testable without external tokenizers. The default analyzer uses Lindera with the embedded Korean dictionary. If tokenizer construction or tokenization fails, conversion returns a typed error rather than silently changing behavior.

`korean_phonemizer` depends on `g2pk`, converts normalized Korean text to IPA, and preserves options for compatibility mode, colloquial `의`, IPA style, and morphology-aware boundary checks. It exposes both the spoken text and IPA string.

`g2pk-py` is a thin PyO3 layer for the G2P converter only. It converts Python arguments into Rust option structs and returns Rust errors as Python exceptions. Phonemizer and Hangulize Python interfaces are separate extension modules.

## Data Flow

Core conversion:

1. Apply idiom substitutions.
2. Convert supported English words to Hangul approximations.
3. Annotate POS-sensitive syllables.
4. Normalize Arabic numerals.
5. Decompose Hangul syllables to canonical jamo.
6. Apply special rules, table rules, linking rules, optional vowel grouping, and optional syllable composition.

Phonemization:

1. Run core conversion to obtain spoken Korean.
2. Decompose each Hangul syllable.
3. Apply IPA mapping and context-sensitive IPA rules.
4. Return `(spoken, ipa)`.

## Error Handling

Public functions return `Result` in Rust. Invalid custom tables, tokenizer failures, unsupported phonemizer modes, and invalid IPA style values produce typed errors with concise messages. Python bindings translate these to `ValueError` or `RuntimeError`.

## Testing

Tests are written before implementation for:

- Hangul compose/decompose helpers.
- Numeric normalization.
- Embedded resource parsing and custom resource precedence.
- Core conversion examples including idioms, English, numerals, and pronunciation rules.
- Phonemizer IPA output for basic syllables and context-sensitive cases.
- PyO3 API construction and call signatures when Python extension tests can be built.

## Known Constraints

The first Rust implementation uses a compact built-in English lexicon for deterministic tests and common inference inputs. The English conversion boundary is isolated so a larger dictionary can be added later without changing public APIs.

The workspace currently has very low disk space. Dependency verification may require freeing Cargo cache or other local storage before a full build can complete.
