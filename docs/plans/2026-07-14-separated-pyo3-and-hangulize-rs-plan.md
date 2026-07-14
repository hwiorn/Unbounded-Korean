# Separated PyO3 Modules and Hangulize-RS Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-dev` skill
> (recommended) or this skill's EXECUTE mode. Steps use `- [ ]` syntax for tracking.

**Goal:** Split Python extension modules by domain and add a Rust `hangulize-rs` core crate.
**Architecture:** Core transliteration lives in `hangulize-rs`; Python wrappers are separate PyO3 crates. Existing `g2pk-py` remains focused on G2P only.
**Tech Stack:** Rust 2024, regex, once_cell, unicode-normalization, PyO3.
**Spec:** docs/specs/2026-07-14-separated-pyo3-and-hangulize-rs-design.md
**Allium:** docs/requirements/2026-07-14-separated-pyo3-and-hangulize-rs.allium

---

## File Map

- `Cargo.toml`: add new workspace crates and shared dependencies.
- `crates/korean_phonemizer-py`: separate Python extension for Korean phonemization.
- `crates/hangulize-rs`: Rust transliteration core, embedded HSL specs, parser, HRE compiler, pipeline, and tests.
- `crates/hangulize-py`: separate Python extension for Hangulize APIs.
- `tests/hangulize_cases.rs`: workspace behavior tests for public Rust API.

## Tasks

### Task 1: RED Tests and Workspace Boundaries

- [ ] **Step 1: Write failing tests** for separate PyO3 crates and `hangulize-rs` public API.
- [ ] **Step 2: Run tests and confirm failure.**

### Task 2: Korean Phonemizer PyO3

- [ ] **Step 1: Implement `korean_phonemizer-py`.**
- [ ] **Step 2: Run `cargo check -p korean_phonemizer-py --features extension-module`.**

### Task 3: Hangulize Core

- [ ] **Step 1: Implement HSL parsing and spec loading.**
- [ ] **Step 2: Implement HRE rule matching and replacements.**
- [ ] **Step 3: Implement pipeline and Hangul composition.**
- [ ] **Step 4: Run `cargo test -p hangulize-rs`.**

### Task 4: Hangulize PyO3

- [ ] **Step 1: Implement `hangulize-py`.**
- [ ] **Step 2: Run `cargo check -p hangulize-py --features extension-module`.**

### Task 5: Final Verification

- [ ] **Step 1: Run formatting for first-party crates.**
- [ ] **Step 2: Run `cargo test --workspace`.**
- [ ] **Step 3: Build/check all PyO3 extension crates.**
- [ ] **Step 4: Validate Allium requirements against tests or explicit command output.**
