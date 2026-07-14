# g2pk-rs Complete Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-dev` skill
> (recommended) or this skill's EXECUTE mode. Steps use `- [ ]` syntax for tracking.

**Goal:** Implement a complete Rust workspace for Korean G2P conversion, Python bindings, and Korean IPA phonemization.
**Architecture:** The workspace has separate core crates for `g2pk`, `korean_phonemizer`, and `hangulize-rs`; each Python interface is exposed by its own PyO3 crate. Core resources are embedded with `include_str!`, with per-instance custom resource overrides.
**Tech Stack:** Rust 2024, regex, once_cell, thiserror, Lindera Korean dictionary, PyO3.
**Spec:** docs/specs/2026-07-14-g2pk-rs-complete-port-design.md
**Allium:** docs/requirements/2026-07-14-g2pk-rs-complete-port.allium

---

## File Map

- `Cargo.toml`: workspace and root package metadata.
- `LICENSE`: Apache 2.0 license text.
- `src/lib.rs`: root re-export crate for core and phonemizer APIs.
- `crates/g2pk/Cargo.toml`: core crate manifest.
- `crates/g2pk/src/lib.rs`: public core API and module exports.
- `crates/g2pk/src/error.rs`: typed errors.
- `crates/g2pk/src/hangul.rs`: Hangul decomposition and composition.
- `crates/g2pk/src/resources.rs`: embedded and custom resource parsing.
- `crates/g2pk/src/morph.rs`: morphology abstraction and Lindera adapter.
- `crates/g2pk/src/numerals.rs`: Arabic numeral normalization.
- `crates/g2pk/src/english.rs`: English word conversion.
- `crates/g2pk/src/rules.rs`: pronunciation rule execution.
- `crates/g2pk/src/g2p.rs`: converter orchestration.
- `crates/g2pk/src/resources/*`: embedded conversion resources.
- `crates/korean_phonemizer/Cargo.toml`: phonemizer crate manifest.
- `crates/korean_phonemizer/src/lib.rs`: Korean IPA public API.
- `crates/g2pk-py/Cargo.toml`: Python extension manifest.
- `crates/g2pk-py/src/lib.rs`: PyO3 module.
- `tests/compat.rs`: workspace-level behavior tests.

## Tasks

### Task 1: Workspace and Resource Skeleton

- [x] **Step 1: Write failing tests** for embedded resource parsing and custom override behavior.
- [x] **Step 2: Run tests and confirm failure** with missing crates/modules.
- [x] **Step 3: Add workspace manifests, resources, errors, and resource parser.**
- [x] **Step 4: Run tests and confirm resource tests pass.**

### Task 2: Hangul, Numerals, and English Normalization

- [x] **Step 1: Write failing tests** for compose/decompose, numeral spelling, and common English conversion.
- [x] **Step 2: Run tests and confirm failure.**
- [x] **Step 3: Implement helper modules.**
- [x] **Step 4: Run tests and confirm they pass.**

### Task 3: Core Converter

- [x] **Step 1: Write failing tests** for idioms, POS-sensitive annotation, rule-table application, link rules, and public options.
- [x] **Step 2: Run tests and confirm failure.**
- [x] **Step 3: Implement morphology abstraction, Lindera adapter, special rules, table rules, and orchestration.**
- [x] **Step 4: Run core tests and confirm they pass.**

### Task 4: Phonemizer Crate

- [x] **Step 1: Write failing tests** for default IPA, simplified IPA, compatibility mode, colloquial option, and invalid options.
- [x] **Step 2: Run tests and confirm failure.**
- [x] **Step 3: Implement `korean_phonemizer` public API and IPA conversion.**
- [x] **Step 4: Run phonemizer tests and confirm they pass.**

### Task 5: PyO3 Module

- [x] **Step 1: Write failing tests or compile checks** for module construction and `G2p`.
- [x] **Step 2: Run checks and confirm failure.**
- [x] **Step 3: Implement `g2pk_rs` bindings.**
- [x] **Step 4: Run compile/tests when Python build dependencies are available.**

### Task 6: Final Verification

- [x] **Step 1: Run `cargo fmt --check`.**
- [x] **Step 2: Run `cargo test --workspace`.**
- [x] **Step 3: Run `cargo check --workspace` and `cargo check -p g2pk-py --features extension-module`.**
- [x] **Step 4: Validate each Allium rule against tests or documented verification status.**
