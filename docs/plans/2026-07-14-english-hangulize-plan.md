# English Hangulize Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-dev` skill
> (recommended) or this skill's EXECUTE mode. Steps use `- [ ]` syntax for tracking.

**Goal:** Add deterministic English-to-Hangul transliteration through `hangulize("eng", text)`.
**Architecture:** `hangulize-rs` registers a new `eng` language and routes its translit stage through `misaki-rs` with no default espeak dependency. English phoneme output is converted by a Rust policy layer into Hangul jamo and composed by the existing syllabifier.
**Tech Stack:** Rust, `hangulize-rs`, `misaki-rs`, existing HSL spec registry, cargo tests.
**Spec:** docs/specs/2026-07-14-english-hangulize-design.md
**Allium:** docs/requirements/2026-07-14-english-hangulize.allium

---

## File Map

- `Cargo.toml`: add workspace dependency for `misaki-rs`.
- `crates/hangulize-rs/Cargo.toml`: add `misaki-rs` dependency with default features disabled.
- `crates/hangulize-rs/src/specs/eng.hsl`: register English language and representative test matrix.
- `crates/hangulize-rs/src/lib.rs`: add `english_phoneme` translit handler and English phoneme-to-Hangul policy.
- `tests/hangulize_cases.rs`: add explicit English coverage.

## Tasks

### Task 1: English Failing Tests

**Files:**
- Modify: `tests/hangulize_cases.rs`

- [x] **Step 1: Write failing tests for English language listing and representative conversions**
- [x] **Step 2: Run targeted tests and confirm failure because `eng` is not registered**

### Task 2: Dependency and Spec Registration

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/hangulize-rs/Cargo.toml`
- Create: `crates/hangulize-rs/src/specs/eng.hsl`

- [x] **Step 1: Add `misaki-rs` dependency with `default-features = false`**
- [x] **Step 2: Add minimal `eng.hsl` with `translit = "english_phoneme"` and matrix cases**
- [x] **Step 3: Run targeted tests and confirm failure moves to missing translit support**

### Task 3: English Phoneme Provider

**Files:**
- Modify: `crates/hangulize-rs/src/lib.rs`

- [x] **Step 1: Add `english_phoneme` translit branch using `misaki_rs::G2P`**
- [x] **Step 2: Run targeted tests and confirm failure moves to phoneme mapping gaps**

### Task 4: Phoneme-to-Hangul Policy

**Files:**
- Modify: `crates/hangulize-rs/src/lib.rs`

- [x] **Step 1: Implement minimal phoneme scanner and jamo emission for the fixed test matrix**
- [x] **Step 2: Reuse existing `compose_hangul` for final syllable composition**
- [x] **Step 3: Run targeted tests until English cases pass**

### Task 5: Verification and Commit

**Files:**
- All touched files

- [x] **Step 1: Run `cargo fmt` for touched packages**
- [x] **Step 2: Run `cargo test --test hangulize_cases`**
- [x] **Step 3: Run `cargo test --workspace`**
- [x] **Step 4: Run PyO3 feature checks**
- [x] **Step 5: Commit the completed feature**
