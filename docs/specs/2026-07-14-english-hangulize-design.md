# English Hangulize Design

## Goal

Add English-to-Korean transliteration to `hangulize-rs` through the existing `hangulize("eng", text)` API.

## Scope

The feature covers English text to Hangul surface transliteration for common TTS/ASR text normalization use. It does not try to model official loanword orthography exhaustively in the first pass. The implementation must be deterministic, testable, and independent from system `espeak` by default.

## Architecture

`hangulize-rs` gets a new bundled `eng` language. Its transliteration stage uses `misaki-rs` to convert English text into a phoneme string, then converts that phoneme string into Hangul with an English-specific phoneme policy.

`misaki-rs` is added with `default-features = false` so the core crate does not require a system speech backend. A future feature can enable the `espeak` fallback if broader out-of-vocabulary coverage is needed.

## Components

- English provider: wraps `misaki_rs::G2P` with `Language::EnglishUS`.
- Phoneme normalizer: removes stress markers, normalizes known diphthongs and affricates into matchable units, and preserves word/punctuation boundaries.
- Phoneme-to-Hangul policy: maps English phoneme units into Korean compatibility jamo and composes them with the existing Hangul syllabifier.
- `eng.hsl`: registers the language and acts as the bundled spec entry point.

## Behavior

Representative outputs are fixed by tests:

- `hello` -> `헬로`
- `world` -> `월드`
- `google` -> `구글`
- `apple` -> `애플`
- `coffee` -> `커피`
- `text` -> `텍스트`
- `AI` -> `에이아이`

Mixed punctuation and whitespace are preserved in the same style as existing `hangulize-rs` languages.

## Error Handling

If `misaki-rs` returns an error, `hangulize-rs` returns `Error::TranslitNotAvailable` or `Error::InvalidSpec` with the underlying message. Unknown words should not panic; they should either use the no-espeak fallback output from `misaki-rs` or pass through conservative phoneme handling.

## Testing

Tests are added before implementation for:

- English language listing.
- Representative English word and acronym cases.
- Punctuation preservation.
- The full bundled spec matrix, including the new `eng.hsl` test block.

The existing workspace test suite and PyO3 feature checks remain required verification.
