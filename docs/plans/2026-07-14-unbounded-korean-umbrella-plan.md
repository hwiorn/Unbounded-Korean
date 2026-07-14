# Unbounded Korean Umbrella Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-dev` skill
> (recommended) or this skill's EXECUTE mode. Steps use `- [ ]` syntax for tracking.

**Goal:** Provide one Rust crate and one Python distribution named `unbounded-korean`.
**Architecture:** The root Rust crate re-exports existing component crates. A separate PyO3 extension crate exposes an umbrella Python module while existing individual Python extension crates remain unchanged.
**Tech Stack:** Rust workspace, PyO3, maturin, cargo integration tests.
**Spec:** docs/specs/2026-07-14-unbounded-korean-umbrella-design.md
**Allium:** docs/requirements/2026-07-14-unbounded-korean-umbrella.allium

---

## File Map

- `Cargo.toml`: rename root package/lib and add umbrella PyO3 workspace member.
- `src/lib.rs`: keep root re-exports.
- `tests/umbrella.rs`: verify Rust umbrella re-exports.
- `crates/unbounded-korean-py/Cargo.toml`: define PyO3 extension crate.
- `crates/unbounded-korean-py/pyproject.toml`: set Python distribution name.
- `crates/unbounded-korean-py/src/lib.rs`: expose umbrella Python module.

## Tasks

- [x] **Step 1: Write failing Rust umbrella re-export test**
- [x] **Step 2: Rename root Rust package/lib to `unbounded-korean` / `unbounded_korean`**
- [x] **Step 3: Add `unbounded-korean-py` PyO3 crate and `pyproject.toml` metadata**
- [x] **Step 4: Run cargo fmt, workspace tests, and PyO3 checks**
- [x] **Step 5: Run maturin smoke test for `unbounded_korean` if maturin is available**
- [x] **Step 6: Commit completed umbrella changes**
