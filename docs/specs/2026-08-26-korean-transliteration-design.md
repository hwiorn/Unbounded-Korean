# korean-transliteration Design

## Goal

Replace hangulize-rs's misaki-based English hangulization (unreliable for out-of-vocabulary
words and brand-name acronyms; see 2026-08-26 session fixes for SKT/NAVER) with a new,
independent crate built on a statistically-trained G2P model (Phonetisaurus) plus a
hand-written, table-driven P2G (phoneme-to-Hangul) stage. English is the first fully
supported source language; the architecture stays extensible to the other ~40 languages
hangulize-rs already covers, without committing to retraining all of them now.

## Scope

- New Rust crate `crates/korean-transliteration`: loads a trained Phonetisaurus `.fst`
  model, runs G2P inference via the `phonetisaurus-g2p` crate, then converts the resulting
  phoneme string to Hangul via a table-driven P2G stage with exception handling for
  irregular phoneme sequences (missing phonemes producing consecutive consonants or
  consecutive vowels).
- New Python module `crates/korean-transliteration-py` (PyO3 + maturin), following the
  existing `crates/unbounded-korean-py` packaging convention.
- New `scripts/train_phonetisaurus.sh`: offline, one-time training script. Runs on a
  remote Linux x86_64 host (`rares01.rapeech.intra`) inside Docker, not on the local
  arm64 development machine.
- Trained `.fst` models are committed under `data/` (one file per source language,
  e.g. `data/eng.fst`), and embedded into the compiled crate via `include_bytes!`.
- This does **not** remove or modify hangulize-rs's existing `eng` language / misaki
  pipeline. The two pipelines exist side by side until `korean-transliteration` is
  validated against real-world coverage.

## Architecture

```
training (offline, one-time, on rares01.rapeech.intra via Docker):
  data collection scripts → aligned word/phoneme corpus → phonetisaurus-align →
  estimate-ngram → phonetisaurus-arpa2wfst → data/<lang>.fst

runtime (crates/korean-transliteration, pure Rust, no C++/Docker dependency):
  word → [phonetisaurus-g2p: model.g2p(word)] → phoneme string (IPA-derived)
        → [P2G table + exception handling] → Hangul string
```

## Components

### Training pipeline (offline)
- **Data collection** (`scripts/collect_training_data.py` or similar): merges training
  pairs from three sources into one `word<TAB>phoneme1 phoneme2 ...` corpus per language:
  1. `crates/hangulize-rs/src/specs/*.hsl` `test:` blocks (word → Hangul; ~4,217 pairs
     across 40 languages) — supplementary seed data only, not sufficient alone.
  2. `muik/transliteration` `data/source/korean-go.txt` (government-sourced English→Korean
     pairs; Apache-2.0 repo, ~30k+ entries) as the primary bulk source for `eng`.
  3. Bulk-generated (English word, simplified IPA) pairs produced by running a large
     English wordlist through Unbounded-Korean's existing misaki-based English G2P
     (`hangulize_rs::hangulize` internals / `misaki_rs::G2P` directly) — this is the
     scalable source that gives real phoneme supervision, since sources 1–2 only have
     Hangul, not phonemes.
  4. Korean phonological rules (tensification, nasalization, liaison, etc.) are informed
     by reading KoG2P's `rulebook.txt` for understanding only — the rules are
     reimplemented independently (reusing/adapting logic already present in
     `crates/g2pk`), and no GPLv3 source text or data is copied into this repository.
     KoG2Padvanced is excluded entirely (no license grant; its `G2P/Dic` directory does
     not actually contain a pronunciation dictionary).
- **`scripts/train_phonetisaurus.sh`**: given a corpus file, runs the standard Phonetisaurus
  toolchain (`phonetisaurus-align` → `estimate-ngram` → `phonetisaurus-arpa2wfst`, or the
  `phonetisaurus-train` wrapper) inside a Docker container. Primary path: build the
  original C++ toolchain from source inside the container on `rares01.rapeech.intra`
  (native Linux x86_64). Fallback path if the from-source build hits the known OpenFst
  1.8+ incompatibility: `pip install phonetisaurus` (rhasspy's prebuilt manylinux1_x86_64
  wheel, which installs natively on that host).
- **Delivery**: local branch is pushed to `origin` (`git@github.com:hwiorn/Unbounded-Korean.git`);
  the remote host clones/pulls that branch, runs the script inside Docker, and the
  resulting `data/*.fst` files are brought back via git (commit on the remote branch,
  push, pull locally) — no ad hoc `scp`.

### Runtime crate (`crates/korean-transliteration`)
- `Model` type wraps `phonetisaurus_g2p::PhonetisaurusModel`, constructed from
  `include_bytes!("../../data/eng.fst")` at compile time (per-language constant model
  registry, mirroring hangulize-rs's `SPECS` table pattern).
- `transliterate(lang: &str, word: &str) -> Result<String, Error>`: looks up the model
  for `lang`, runs G2P, then applies the P2G table.
- P2G table + exception handling: generalizes the onset/vowel/diphthong composition logic
  already fixed in `hangulize-rs`'s `english_units`/`english_phoneme_word_to_hangul` this
  session. Handles the specific failure modes named in the Allium contract:
  consecutive-consonant runs (missing vowel) and consecutive-vowel runs (missing
  consonant) get deterministic insertion/merge rules rather than producing malformed
  Hangul syllable blocks.
- No panics on out-of-vocabulary input or malformed phoneme output — always `Result`.

### Python bindings (`crates/korean-transliteration-py`)
- Mirrors `crates/unbounded-korean-py`: `pyo3` + `maturin`, `cdylib`+`rlib`, exposes
  `transliterate(lang: str, word: str) -> str`.

## Behavior

Representative outputs fixed by tests (ported from this session's hangulize-rs fixes,
re-verified against the new pipeline):

- `SKT` -> `에스케이티`
- `NAVER` -> `네이버`
- `hello` -> `헬로`
- All existing `tests/hangulize_cases.rs` English assertions must still hold when routed
  through `korean-transliteration`.

## Known Risks / Open Questions (tracked as explicit plan tasks, not silently assumed)

1. **Model accuracy is unverified.** `phonetisaurus-g2p` (crates.io) is a small,
   single-maintainer, low-adoption crate (3 commits, ~1,895 downloads) whose own author
   documents that its FST shortest-path search can be less accurate than the reference
   C++ decoder. A dedicated accuracy-validation task (comparing decoded output against a
   held-out test set) is required before this pipeline can be considered a real
   replacement for the existing hangulize-rs path.
2. **Toolchain build risk on the remote host.** The original Phonetisaurus C++ toolchain
   is known to fail to build against OpenFst 1.8+ (unresolved upstream issue since 2021).
   `rares01.rapeech.intra` is native x86_64 Linux, so the rhasspy PyPI wheel
   (`pip install phonetisaurus`) is a viable fallback if the from-source build fails.
3. **Data scale is English-only for now.** The other ~39 hangulize-rs languages only have
   6–500 seed pairs each (no phoneme annotations at all) — not enough to train a
   meaningful per-language model today. This design intentionally scopes full
   correctness to `eng` and leaves the rest as a structurally-supported but
   not-yet-trained extension point.
