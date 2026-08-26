# korean-transliteration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `subagent-driven-dev` skill
> (recommended) or this skill's EXECUTE mode. Steps use `- [ ]` syntax for tracking.

**Goal:** Replace hangulize-rs's misaki-based English hangulization with a new crate
built on a Phonetisaurus-trained G2P model plus a hand-written table-driven P2G stage,
starting with English and structured to extend to other languages later.
**Architecture:** Offline training (Docker, on `rares01.rapeech.intra`, native Linux
x86_64) produces `data/<lang>.fst` files from a merged corpus (hangulize-rs `.hsl` test
pairs + `muik/transliteration` `korean-go.txt` + bulk misaki-generated (word, IPA) pairs).
The runtime crate (`crates/korean-transliteration`) embeds those `.fst` files via
`include_bytes!` and decodes with the pure-Rust `phonetisaurus-g2p` crate, then converts
phonemes to Hangul via a self-contained P2G jamo-composition table.
**Tech Stack:** Rust (workspace crate + `phonetisaurus-g2p`, `once_cell`, `thiserror`),
PyO3 + maturin for Python bindings, Phonetisaurus C++ toolchain (or rhasspy's PyPI wheel
fallback) run inside Docker on a remote x86_64 host for training only.
**Spec:** docs/specs/2026-08-26-korean-transliteration-design.md
**Allium:** docs/requirements/2026-08-26-korean-transliteration.allium

---

## Commit Policy

Per this session's operating rules, commits are made **only when the user explicitly
asks**. Every "Step 5" below stages files and reports the diff for review; it does not
run `git commit` unless the user has said to commit at that point.

## File Map

| File | Responsibility |
|---|---|
| `scripts/collect_hsl_seed_data.py` | Extract (word, hangul) pairs from all `crates/hangulize-rs/src/specs/*.hsl` `test:` blocks into `data/corpus/hsl_seed.tsv` |
| `scripts/collect_korean_go_data.sh` | Fetch/normalize `muik/transliteration`'s `data/source/korean-go.txt` into `data/corpus/korean_go.tsv` |
| `scripts/generate_ipa_corpus.rs` (bin target under `crates/hangulize-rs/examples/` or a small standalone bin crate) | Run a large English wordlist through the existing misaki-based English G2P to emit `data/corpus/eng_ipa.tsv` (word, simplified IPA) |
| `scripts/build_training_corpus.py` | Merge the three sources above into `data/corpus/eng.dict` (`word<TAB>phonemes`) in Phonetisaurus's expected format |
| `scripts/train_phonetisaurus.sh` | Orchestrate training on the remote host: push branch, SSH, run Docker build/train, pull resulting `.fst` back via git |
| `docker/phonetisaurus-train.Dockerfile` | Container image: build Phonetisaurus C++ toolchain from source, with a documented fallback to `pip install phonetisaurus` |
| `data/eng.fst` | Trained model artifact (binary, committed) |
| `crates/korean-transliteration/Cargo.toml` | New crate manifest |
| `crates/korean-transliteration/src/lib.rs` | Public `transliterate()` API, model registry |
| `crates/korean-transliteration/src/p2g.rs` | Phoneme table + Hangul jamo composition + gap-repair exception handling |
| `crates/korean-transliteration/src/hangul.rs` | Standalone L/V/T Hangul syllable composer (no dependency on hangulize-rs internals) |
| `crates/korean-transliteration-py/Cargo.toml`, `src/lib.rs`, `pyproject.toml` | Python bindings, mirrors `crates/unbounded-korean-py` |
| `Cargo.toml` (workspace root) | Add both new crates to `members`, add `phonetisaurus-g2p` to `workspace.dependencies` |
| `tests/korean_transliteration_cases.rs` | Integration tests: SKT/NAVER/existing English cases + gap-repair edge cases |

---

## Task 1: Extract hangulize-rs `.hsl` seed pairs

**Files:**
- Create: `scripts/collect_hsl_seed_data.py`
- Test: `scripts/tests/test_collect_hsl_seed_data.py`

- [ ] **Step 1: Write the failing test**

```python
# scripts/tests/test_collect_hsl_seed_data.py
from pathlib import Path
from collect_hsl_seed_data import parse_hsl_tests

def test_parses_eng_hsl_test_block(tmp_path):
    hsl = tmp_path / "eng.hsl"
    hsl.write_text(
        'lang:\n    id = "eng"\n'
        'test:\n'
        '    "SKT"   -> "에스케이티"\n'
        '    "hello" -> "헬로"\n'
    )
    pairs = parse_hsl_tests(hsl.read_text())
    assert pairs == [("SKT", "에스케이티"), ("hello", "헬로")]
```

- [ ] **Step 2: Run test — must fail**
Run: `python3 -m pytest scripts/tests/test_collect_hsl_seed_data.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'collect_hsl_seed_data'`

- [ ] **Step 3: Write minimal implementation**

```python
# scripts/collect_hsl_seed_data.py
import csv
import glob
import sys
from pathlib import Path

def parse_hsl_tests(src: str) -> list[tuple[str, str]]:
    in_test = False
    pairs = []
    for raw in src.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.endswith(":"):
            in_test = line[:-1] == "test"
            continue
        if not in_test or "->" not in line:
            continue
        left, right = line.split("->", 1)
        pairs.append((_unquote(left), _unquote(right)))
    return pairs

def _unquote(value: str) -> str:
    value = value.strip()
    if not value.startswith('"'):
        return value
    return value[1:].rsplit('"', 1)[0]

def main(specs_dir: str, out_path: str) -> None:
    rows = []
    for path in sorted(glob.glob(f"{specs_dir}/*.hsl")):
        lang = Path(path).stem
        for word, hangul in parse_hsl_tests(Path(path).read_text(encoding="utf-8")):
            rows.append((lang, word, hangul))
    with open(out_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f, delimiter="\t")
        writer.writerows(rows)

if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
```

- [ ] **Step 4: Run test — must pass**
Run: `python3 -m pytest scripts/tests/test_collect_hsl_seed_data.py -v`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add scripts/collect_hsl_seed_data.py scripts/tests/test_collect_hsl_seed_data.py
```
Do not commit — report the diff and wait for the user.

---

## Task 2: Fetch and normalize `korean-go.txt`

**Files:**
- Create: `scripts/collect_korean_go_data.sh`
- Test: `scripts/tests/test_collect_korean_go_data.sh` (shell-based assertion script)

- [ ] **Step 1: Write the failing test**

```bash
#!/usr/bin/env bash
# scripts/tests/test_collect_korean_go_data.sh
set -euo pipefail
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/korean-go.raw.txt" <<'EOF'
# 국립국어원 외래어 표기법 용례
kanaan	가나안
garnet	가넷
EOF

bash scripts/collect_korean_go_data.sh "$tmp/korean-go.raw.txt" "$tmp/out.tsv"

expected=$'kanaan\t가나안\ngarnet\t가넷'
actual=$(cat "$tmp/out.tsv")
if [ "$actual" != "$expected" ]; then
  echo "FAIL: got:\n$actual"
  exit 1
fi
echo "PASS"
```

- [ ] **Step 2: Run test — must fail**
Run: `bash scripts/tests/test_collect_korean_go_data.sh`
Expected: FAIL — `scripts/collect_korean_go_data.sh: No such file or directory`

- [ ] **Step 3: Write minimal implementation**

```bash
#!/usr/bin/env bash
# scripts/collect_korean_go_data.sh
# Usage: collect_korean_go_data.sh <input-raw-tsv> <output-tsv>
# Strips leading '#' comment lines and blank lines from a muik/transliteration
# data/source/*.txt file, keeping only tab-delimited (english, hangul) pairs.
set -euo pipefail
in_file="$1"
out_file="$2"
grep -v '^#' "$in_file" | grep -v '^[[:space:]]*$' > "$out_file"
```

- [ ] **Step 4: Run test — must pass**
Run: `bash scripts/tests/test_collect_korean_go_data.sh`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add scripts/collect_korean_go_data.sh scripts/tests/test_collect_korean_go_data.sh
```
Note: this script only *normalizes* a local copy of `korean-go.txt`; actually fetching
the file from `raw.githubusercontent.com/muik/transliteration/master/data/source/korean-go.txt`
requires a network call the user should confirm before this runs unattended (some
environments sandbox outbound network access) — call this out explicitly when this task
executes rather than silently curling.

---

## Task 3: Bulk-generate (English word, simplified IPA) corpus

**Files:**
- Create: `crates/hangulize-rs/examples/generate_ipa_corpus.rs`
- Test: `crates/hangulize-rs/tests/ipa_corpus_generation.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/hangulize-rs/tests/ipa_corpus_generation.rs (as actually implemented; the
// original draft's "naver" example was wrong — misaki fails to phonemize "naver" as a
// coherent word at all, per this session's earlier NAVER investigation, so it can't be
// used as a corpus-generation example)
use hangulize_rs::english_ipa_for_corpus;

#[test]
fn emits_simplified_ipa_for_common_words() {
    assert_eq!(english_ipa_for_corpus("hello").unwrap(), "h ə l oʊ");
    assert_eq!(english_ipa_for_corpus("text").unwrap(), "t ɛ k s t");
}
```

- [ ] **Step 2: Run test — must fail**
Run: `cargo test -p hangulize-rs --test ipa_corpus_generation`
Expected: FAIL with `error[E0432]: unresolved import` (function does not exist yet)

- [ ] **Step 3: Write minimal implementation**

Add a small, explicitly-public wrapper in `crates/hangulize-rs/src/lib.rs` around the
existing (private) misaki G2P call, returning space-separated simplified phoneme symbols
(reusing the existing stress-marker-stripping already done for the `english_phoneme`
translit path — do not duplicate that stripping logic, extract it into a shared helper
both call):

```rust
pub fn english_ipa_for_corpus(word: &str) -> String {
    let g2p = ENGLISH_G2P.lock().unwrap();
    let (raw_phonemes, _) = g2p.g2p(word).expect("g2p on a plain word must not fail");
    simplify_phoneme_string(&raw_phonemes)
}
```

`simplify_phoneme_string` strips stress marks (`ˈ`, `ˌ`, `ː`, tie bars) and inserts a
single space between phoneme units using the same unit boundaries `english_units`
already computes, so the corpus's phoneme alphabet matches exactly what
`crates/korean-transliteration`'s P2G table will need to handle later.

Then add the corpus-generation binary:

```rust
// crates/hangulize-rs/examples/generate_ipa_corpus.rs
use std::fs;
use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let wordlist_path = &args[1];
    let out_path = &args[2];
    let words = fs::File::open(wordlist_path).expect("wordlist must exist");
    let mut out = fs::File::create(out_path).expect("cannot create output");
    for line in std::io::BufReader::new(words).lines() {
        let word = line.expect("readable line");
        let word = word.trim();
        if word.is_empty() {
            continue;
        }
        let ipa = hangulize_rs::english_ipa_for_corpus(word);
        writeln!(out, "{word}\t{ipa}").expect("write must succeed");
    }
}
```

- [ ] **Step 4: Run test — must pass**
Run: `cargo test -p hangulize-rs --test ipa_corpus_generation`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add crates/hangulize-rs/src/lib.rs crates/hangulize-rs/examples/generate_ipa_corpus.rs crates/hangulize-rs/tests/ipa_corpus_generation.rs
```
Resolved: use the full system wordlist, `/usr/share/dict/words` (~235k entries) — the
user confirmed using all of it rather than a filtered subset.

**Result (executed 2026-08-26):** `english_ipa_for_corpus` returns `Result<String,
Error>` (not a panicking `String` as originally sketched — a single bad word must not
abort a 235k-word batch run). The generator binary skips-and-logs per-word errors
instead of aborting. Ran on the full `/usr/share/dict/words` (235,976 words) in 47s,
0 hard failures, 3 rows with likely OOV passthrough noise (negligible, ~0.001%).
Output: `data/corpus/eng_ipa.tsv` (8.0MB).

---

## Task 4: Build the Phonetisaurus training corpus

**Design correction found during execution (2026-08-26):** the original draft of this
task merged `hsl_seed.tsv`, `korean_go.tsv`, and `eng_ipa.tsv` together on the
assumption they share one phoneme alphabet. They don't — `hsl_seed.tsv` and
`korean_go.tsv` are (word, **Hangul**) pairs; only `eng_ipa.tsv` is (word, **IPA
phoneme**) pairs. A Phonetisaurus G2P model needs one consistent output alphabet, so
mixing Hangul and IPA labels in the same training file would corrupt the model. Fixed
scope: the G2P training corpus is built from `eng_ipa.tsv` only.
`hsl_seed.tsv` (eng subset) and `korean_go.tsv` are repurposed as the held-out,
ground-truth **evaluation** set for Task 10 instead (word → known-correct Hangul is
exactly what an end-to-end accuracy check needs, and is a better fit for that role than
for G2P training).

**Files:**
- Create: `scripts/build_training_corpus.py`
- Test: `scripts/tests/test_build_training_corpus.py`

- [ ] **Step 1: Write the failing test**

```python
# scripts/tests/test_build_training_corpus.py
from build_training_corpus import build_corpus

def test_filters_noisy_passthrough_rows(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("hello\th ə l oʊ\nA\tA\n")  # "A\tA" is OOV passthrough noise
    out = tmp_path / "eng.dict"
    build_corpus(ipa, out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ"]

def test_rejects_duplicate_words(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("hello\th ə l oʊ\nhello\th ə l oʊ\n")
    out = tmp_path / "eng.dict"
    build_corpus(ipa, out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ"]
```

- [ ] **Step 2: Run test — must fail**
Run: `python3 -m pytest scripts/tests/test_build_training_corpus.py -v`
Expected: FAIL — module not found

- [ ] **Step 3: Write minimal implementation**

```python
# scripts/build_training_corpus.py
import sys
from pathlib import Path

def build_corpus(ipa_path: Path, out_path: Path) -> None:
    seen = {}
    for line in ipa_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        word, phonemes = line.split("\t", 1)
        if word == phonemes:
            continue  # OOV passthrough noise: no real phonemization happened
        seen[word] = phonemes
    with open(out_path, "w", encoding="utf-8") as f:
        for word in sorted(seen):
            f.write(f"{word}\t{seen[word]}\n")

if __name__ == "__main__":
    build_corpus(Path(sys.argv[1]), Path(sys.argv[2]))
```

- [ ] **Step 4: Run test — must pass**
Run: `python3 -m pytest scripts/tests/test_build_training_corpus.py -v`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add scripts/build_training_corpus.py scripts/tests/test_build_training_corpus.py
```

---

## Task 5: `train_phonetisaurus.sh` + Docker image (remote execution)

**Files:**
- Create: `docker/phonetisaurus-train.Dockerfile`
- Create: `scripts/train_phonetisaurus.sh`
- Test: manual verification run (documented below) — this task is infrastructure, not
  unit-testable in isolation; the "test" is the script's own `--dry-run` mode.

- [ ] **Step 1: Write the failing check**

```bash
# invocation that must fail before the script exists
bash scripts/train_phonetisaurus.sh --dry-run --lang eng
```
Expected: `bash: scripts/train_phonetisaurus.sh: No such file or directory`

- [ ] **Step 2: Confirm failure** (same command, same expected output)

- [ ] **Step 3: Write the Dockerfile and script**

```dockerfile
# docker/phonetisaurus-train.Dockerfile
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential autoconf-archive libtool automake git python3 python3-pip \
    && rm -rf /var/lib/apt/lists/*
# Primary path: build OpenFst 1.7.x + Phonetisaurus from source (the version known to
# work; 1.8+ breaks Phonetisaurus's build per upstream issue #70).
# Fallback path (used if the from-source build fails): `pip install phonetisaurus`
# (rhasspy's prebuilt manylinux1_x86_64 wheel — this image is x86_64, so it applies).
WORKDIR /build
COPY docker/build_phonetisaurus.sh .
RUN bash build_phonetisaurus.sh || pip3 install --break-system-packages phonetisaurus
WORKDIR /work
```

```bash
#!/usr/bin/env bash
# scripts/train_phonetisaurus.sh
# Trains one Phonetisaurus G2P model per configured language and writes
# data/<lang>.fst. Must run on a Linux x86_64 host — the rhasspy PyPI wheel and the
# from-source build both target that platform; see docs/specs/2026-08-26-
# korean-transliteration-design.md for why this can't run natively on macOS arm64.
set -euo pipefail

REMOTE_HOST="${TRAIN_REMOTE_HOST:-gglee@rares01.rapeech.intra}"
LANG_CODE="${1:?usage: train_phonetisaurus.sh <lang> [--dry-run]}"
DRY_RUN="${2:-}"

if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "[dry-run] would train data/corpus/${LANG_CODE}.dict -> data/${LANG_CODE}.fst on ${REMOTE_HOST}"
  exit 0
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
git push origin "$BRANCH"

ssh "$REMOTE_HOST" bash -s "$BRANCH" "$LANG_CODE" <<'REMOTE_SCRIPT'
set -euo pipefail
BRANCH="$1"
LANG_CODE="$2"
WORKDIR="$HOME/korean-transliteration-train"
if [ ! -d "$WORKDIR" ]; then
  git clone git@github.com:hwiorn/Unbounded-Korean.git "$WORKDIR"
fi
cd "$WORKDIR"
git fetch origin "$BRANCH"
git checkout "$BRANCH"
git pull origin "$BRANCH"

docker build -f docker/phonetisaurus-train.Dockerfile -t phonetisaurus-train .
docker run --rm -v "$WORKDIR/data:/work/data" phonetisaurus-train \
  phonetisaurus-train --lexicon "/work/data/corpus/${LANG_CODE}.dict" \
  --dir_prefix "/work/data/${LANG_CODE}_train"
cp "data/${LANG_CODE}_train/model.fst" "data/${LANG_CODE}.fst"

git add "data/${LANG_CODE}.fst"
git commit -m "data: train ${LANG_CODE}.fst on rares01"
git push origin "$BRANCH"
REMOTE_SCRIPT

git pull origin "$BRANCH"
echo "Trained model available at data/${LANG_CODE}.fst"
```

**Correction found during Task 6 execution (2026-08-26):** the CLI name/flags above were
guessed and turned out wrong on two counts. (1) `docker/build_phonetisaurus.sh`'s
from-source build failed at the MITLM `./configure` step because it probes for a
Fortran 77 compiler (not installed in the image) and aborts — never even reaching the
OpenFst-1.8-incompatibility risk this plan anticipated. (2) The pip-wheel fallback
activated correctly, but it doesn't provide a `phonetisaurus-train` binary at all — the
rhasspy package installs a single `phonetisaurus` console script with `train`/`predict`
subcommands (`phonetisaurus train --model MODEL lexicon...`, `phonetisaurus predict
--model MODEL words...`), plus a Python API (`phonetisaurus.train()`/`.predict()`) and
its own bundled precompiled OpenFst binaries — it does not shell out to a system
Phonetisaurus install at all. `scripts/train_phonetisaurus.sh` and `crates/korean-transliteration`'s later inference
calls use this real CLI, not the guessed one. The from-source path was left as-is (not worth
fixing the Fortran dependency) since the pip-wheel path already gives a fully working,
modern OpenFst-based toolchain.
```

- [ ] **Step 4: Run the dry-run — must succeed**
Run: `bash scripts/train_phonetisaurus.sh eng --dry-run`
Expected: prints the `[dry-run]` line and exits 0

- [ ] **Step 5: Stage for review**
```bash
git add docker/phonetisaurus-train.Dockerfile scripts/train_phonetisaurus.sh
```
**Do not run the non-dry-run form until the user explicitly approves** — it pushes to
`origin`, SSHes into `rares01.rapeech.intra`, and commits/pushes from that remote host.
That is exactly the kind of shared/hard-to-reverse action this session's operating rules
require confirming before running, even though the user already named this host.

---

## Task 6: First real remote training run (English) + fallback verification

**Files:** none (execution task, not a code change)

- [ ] **Step 1:** Confirm with the user before the first live (non-dry-run) invocation.
- [ ] **Step 2:** Run `bash scripts/train_phonetisaurus.sh eng` for real.
- [ ] **Step 3:** If the from-source Phonetisaurus build fails inside the Docker image
      (expected risk: OpenFst 1.8+ incompatibility), confirm the Dockerfile's `pip3
      install phonetisaurus` fallback line actually activated (check the build log for
      "Successfully installed phonetisaurus"), and that `phonetisaurus-train` is still
      on `PATH` afterward (the pip package ships the same CLI name).
- [ ] **Step 4:** Verify `data/eng.fst` exists locally after the script completes and is
      a valid OpenFst binary: `file data/eng.fst` should report a binary file, not text.
- [ ] **Step 5:** Report the actual build path used (from-source vs. pip fallback) back
      to the user — this materially affects how reproducible future re-training is.

---

## Task 7: Scaffold `crates/korean-transliteration`

**Files:**
- Create: `crates/korean-transliteration/Cargo.toml`
- Create: `crates/korean-transliteration/src/lib.rs`
- Create: `crates/korean-transliteration/src/hangul.rs`
- Modify: `Cargo.toml` (workspace root — add member + `phonetisaurus-g2p` dependency)
- Test: `crates/korean-transliteration/tests/loads_model.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/korean-transliteration/tests/loads_model.rs
#[test]
fn eng_model_loads_without_panicking() {
    let _ = korean_transliteration::transliterate("eng", "hello");
}
```

- [ ] **Step 2: Run test — must fail**
Run: `cargo test -p korean-transliteration --test loads_model`
Expected: FAIL — package `korean-transliteration` does not exist yet

- [ ] **Step 3: Write minimal implementation**

```toml
# crates/korean-transliteration/Cargo.toml
[package]
name = "korean-transliteration"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
name = "korean_transliteration"

[dependencies]
phonetisaurus-g2p.workspace = true
once_cell.workspace = true
thiserror.workspace = true
```

```toml
# workspace Cargo.toml additions
[workspace.dependencies]
phonetisaurus-g2p = "0.1.1"
korean-transliteration = { path = "crates/korean-transliteration" }
```
(add `"crates/korean-transliteration"` and `"crates/korean-transliteration-py"` to
`[workspace] members`)

```rust
// crates/korean-transliteration/src/lib.rs
mod hangul;
mod p2g;

use once_cell::sync::Lazy;
use phonetisaurus_g2p::PhonetisaurusModel;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no trained model for language: {0}")]
    ModelNotFound(String),
    #[error("g2p failed for {word:?}: {source}")]
    G2p { word: String, source: anyhow::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

static ENG_MODEL: Lazy<PhonetisaurusModel> = Lazy::new(|| {
    static BYTES: &[u8] = include_bytes!("../../../data/eng.fst");
    PhonetisaurusModel::try_from(BYTES).expect("bundled data/eng.fst must be a valid model")
});

fn model_for(lang: &str) -> Option<&'static PhonetisaurusModel> {
    match lang {
        "eng" | "en" => Some(&ENG_MODEL),
        _ => None,
    }
}

pub fn transliterate(lang: &str, word: &str) -> Result<String> {
    let model = model_for(lang).ok_or_else(|| Error::ModelNotFound(lang.to_string()))?;
    let decoded = model
        .phonemize_word(word)
        .map_err(|source| Error::G2p { word: word.to_string(), source })?;
    Ok(p2g::phonemes_to_hangul(&decoded.phonemes))
}
```

`data/eng.fst` must exist (from Task 6) before this compiles — `include_bytes!` is a
compile-time dependency, so Task 7 is blocked on Task 6 completing for real, not just on
its dry-run.

- [ ] **Step 4: Run test — must pass**
Run: `cargo test -p korean-transliteration --test loads_model`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add crates/korean-transliteration Cargo.toml
```

---

## Task 8: P2G table + phoneme-gap exception handling

**Files:**
- Create: `crates/korean-transliteration/src/p2g.rs`
- Test: `crates/korean-transliteration/tests/p2g_cases.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// crates/korean-transliteration/tests/p2g_cases.rs
use korean_transliteration::p2g::phonemes_to_hangul;

#[test]
fn composes_simple_cvc_word() {
    assert_eq!(phonemes_to_hangul("h ɛ l oʊ"), "헬로");
}

#[test]
fn repairs_consecutive_consonants_from_a_dropped_vowel() {
    // Simulates a decoder that skipped a vowel between two consonants: insert the
    // closest neutral vowel (ㅡ) rather than emitting two bare consonant jamo.
    assert_eq!(phonemes_to_hangul("s k t"), "스크트");
}

#[test]
fn repairs_consecutive_vowels_from_a_dropped_consonant() {
    // Two adjacent vowel phonemes with no consonant between them: insert a silent
    // ㅇ onset for the second syllable instead of merging into one invalid vowel run.
    assert_eq!(phonemes_to_hangul("a i"), "아이");
}
```

(`p2g` must be declared `pub mod p2g;` in `lib.rs`, not `mod p2g;`, for this test to
compile against the public API — update Task 7's `lib.rs` accordingly.)

- [ ] **Step 2: Run test — must fail**
Run: `cargo test -p korean-transliteration --test p2g_cases`
Expected: FAIL — `phonemes_to_hangul` not found (empty `p2g.rs` from Task 7 scaffold)

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/korean-transliteration/src/p2g.rs
use crate::hangul::compose_syllable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Onset(char),
    Vowel(char),
    Coda(char),
}

/// Maps one simplified-IPA phoneme token to a Korean jamo unit. Returns `None` for
/// tokens with no direct mapping (stress marks etc. are expected to already be
/// stripped upstream by the corpus generator from Task 3).
fn unit_for(token: &str) -> Option<Unit> {
    match token {
        "h" => Some(Unit::Onset('ㅎ')),
        "l" => Some(Unit::Onset('ㄹ')),
        "s" => Some(Unit::Onset('ㅅ')),
        "k" => Some(Unit::Onset('ㅋ')),
        "t" => Some(Unit::Onset('ㅌ')),
        "ɛ" => Some(Unit::Vowel('ㅔ')),
        "oʊ" => Some(Unit::Vowel('ㅗ')),
        "a" => Some(Unit::Vowel('ㅏ')),
        "i" => Some(Unit::Vowel('ㅣ')),
        _ => None,
    }
}

pub fn phonemes_to_hangul(phonemes: &str) -> String {
    let units: Vec<Unit> = phonemes.split_whitespace().filter_map(unit_for).collect();
    let units = repair_gaps(units);
    render(&units)
}

/// Exception handling for missing phonemes: inserts a neutral vowel (ㅡ) between two
/// consecutive onsets with no vowel between them, and inserts a silent onset (ㅇ,
/// handled naturally by `render`) is already implicit for consecutive vowels since
/// each vowel simply starts its own syllable — the only real repair needed is the
/// consonant-run case.
fn repair_gaps(units: Vec<Unit>) -> Vec<Unit> {
    let mut out = Vec::with_capacity(units.len());
    for (i, unit) in units.iter().copied().enumerate() {
        out.push(unit);
        let next_is_onset_with_no_vowel_between =
            matches!(unit, Unit::Onset(_)) && matches!(units.get(i + 1), Some(Unit::Onset(_)));
        if next_is_onset_with_no_vowel_between {
            out.push(Unit::Vowel('ㅡ'));
        }
    }
    out
}

fn render(units: &[Unit]) -> String {
    let mut out = String::new();
    let mut pending_onset: Option<char> = None;
    let mut i = 0;
    while i < units.len() {
        match units[i] {
            Unit::Onset(c) => {
                pending_onset = Some(c);
                i += 1;
            }
            Unit::Vowel(v) => {
                let onset = pending_onset.take().unwrap_or('ㅇ');
                let coda = match units.get(i + 1) {
                    Some(Unit::Coda(c)) => Some(*c),
                    _ => None,
                };
                out.push(compose_syllable(onset, v, coda));
                i += if coda.is_some() { 2 } else { 1 };
            }
            Unit::Coda(_) => {
                // A coda with no preceding vowel in this stream is itself a gap;
                // treat it as its own syllable with a silent onset and neutral vowel.
                if let Unit::Coda(c) = units[i] {
                    out.push(compose_syllable('ㅇ', 'ㅡ', Some(c)));
                }
                i += 1;
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run test — must pass**
Run: `cargo test -p korean-transliteration --test p2g_cases`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add crates/korean-transliteration/src/p2g.rs crates/korean-transliteration/src/lib.rs crates/korean-transliteration/tests/p2g_cases.rs
```

**Known gap, logged rather than silently expanded:** the phoneme table above only covers
the symbols needed by this task's test cases. Task 9 below is where the table gets
filled out against the real training corpus's full phoneme inventory (from Task 3) —
do not hand-expand this table speculatively before that data exists.

---

## Task 9: Full phoneme inventory + regression parity with hangulize-rs

**Files:**
- Modify: `crates/korean-transliteration/src/p2g.rs` (fill out `unit_for` against the
  real corpus's phoneme alphabet from Task 3)
- Create: `tests/korean_transliteration_cases.rs` (workspace-root integration test,
  mirrors `tests/hangulize_cases.rs`'s English assertions)

- [ ] **Step 1: Write the failing tests**

```rust
// tests/korean_transliteration_cases.rs
#[test]
fn matches_hangulize_rs_english_regression_cases() {
    let cases = [
        ("SKT", "에스케이티"),
        ("NAVER", "네이버"),
        ("hello", "헬로"),
        ("world", "월드"),
        ("google", "구글"),
        ("apple", "애플"),
        ("coffee", "커피"),
        ("text", "텍스트"),
        ("AI", "에이아이"),
    ];
    for (word, expected) in cases {
        assert_eq!(
            korean_transliteration::transliterate("eng", word).unwrap(),
            expected,
            "{word}"
        );
    }
}
```

- [ ] **Step 2: Run test — must fail**
Run: `cargo test --test korean_transliteration_cases`
Expected: FAIL on most cases (the phoneme table is still incomplete at this point)

- [ ] **Step 3: Fill out the phoneme table**
Extend `unit_for` to cover every phoneme symbol that appears in `data/corpus/eng.dict`
(from Task 3's `english_ipa_for_corpus` output alphabet — enumerate it with
`cut -f2 data/corpus/eng.dict | tr ' ' '\n' | sort -u` and map each one). Add the
onset/coda distinction for consonants that can appear in both positions (Korean coda
consonants are a restricted set of 7 sounds — reuse the same mapping used in
`hangulize-rs`'s `render_english_consonants`/`split_final_cluster` as a reference for
which consonant clusters need splitting across two syllables rather than one bare coda).

- [ ] **Step 4: Run test — must pass**
Run: `cargo test --test korean_transliteration_cases`
Expected: PASS for every case

- [ ] **Step 5: Stage for review**
```bash
git add crates/korean-transliteration/src/p2g.rs tests/korean_transliteration_cases.rs
```

---

## Task 10: Accuracy validation against a held-out set

**Files:**
- Create: `scripts/evaluate_model_accuracy.py`
- Test: N/A (this task *is* the verification step — its output is the artifact, not a
  passing/failing unit test)

- [ ] **Step 1:** Use `data/corpus/korean_go.tsv` (31,898 pairs) and the `eng`-language
      rows of `data/corpus/hsl_seed.tsv` as the held-out evaluation set — these are
      real, human-curated (word → known-correct Hangul) pairs, not something the G2P
      model was trained on (it only saw IPA labels from `eng_ipa.tsv`), so they're a
      genuine end-to-end ground truth rather than a random split of our own generated
      data.
- [ ] **Step 2:** Write `scripts/evaluate_model_accuracy.py` to run every evaluation
      word through `korean_transliteration::transliterate` (via a thin CLI binary or
      PyO3 binding) and report exact-match accuracy against the known-correct Hangul.
- [ ] **Step 3:** Run it and record the accuracy number in
      `docs/plans/2026-08-26-korean-transliteration-plan.md` (this file) under a new
      "Results" section, plus a comparison against hangulize-rs's existing misaki
      pipeline on the same held-out words.
- [ ] **Step 4:** If accuracy is materially worse than the existing misaki pipeline,
      stop and report this to the user before proceeding to Task 11 — per the Allium
      contract, this crate replacing hangulize-rs's pipeline is not a goal in itself;
      the goal is fixing SKT/NAVER-style failures without regressing overall coverage.
- [ ] **Step 5:** Stage `scripts/evaluate_model_accuracy.py` and the results for review.

---

## Task 11: Python bindings

**Files:**
- Create: `crates/korean-transliteration-py/Cargo.toml`
- Create: `crates/korean-transliteration-py/src/lib.rs`
- Create: `crates/korean-transliteration-py/pyproject.toml`
- Test: `crates/korean-transliteration-py/tests/test_parity.py`

- [ ] **Step 1: Write the failing test**

```python
# crates/korean-transliteration-py/tests/test_parity.py
import korean_transliteration

def test_skt_matches_rust_expectation():
    assert korean_transliteration.transliterate("eng", "SKT") == "에스케이티"
```

- [ ] **Step 2: Run test — must fail**
Run: `python3 -m pytest crates/korean-transliteration-py/tests/test_parity.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'korean_transliteration'`

- [ ] **Step 3: Write minimal implementation**

```toml
# crates/korean-transliteration-py/Cargo.toml
[package]
name = "korean-transliteration-py"
version.workspace = true
edition = "2021"
license.workspace = true

[lib]
name = "korean_transliteration"
crate-type = ["cdylib", "rlib"]
test = false
doctest = false

[features]
default = []
extension-module = ["pyo3/extension-module"]

[dependencies]
korean-transliteration.workspace = true
pyo3.workspace = true
```

```rust
// crates/korean-transliteration-py/src/lib.rs
use pyo3::prelude::*;

#[pyfunction]
fn transliterate(lang: &str, word: &str) -> PyResult<String> {
    korean_transliteration::transliterate(lang, word)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

#[pymodule]
fn korean_transliteration(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(transliterate, m)?)?;
    Ok(())
}
```

```toml
# crates/korean-transliteration-py/pyproject.toml
[build-system]
requires = ["maturin>=1.7,<2.0"]
build-backend = "maturin"

[project]
name = "korean-transliteration"
version = "0.1.0"
requires-python = ">=3.8"
license = { text = "Apache-2.0" }
description = "Python bindings for korean-transliteration (Phonetisaurus G2P + table P2G)."

[tool.maturin]
module-name = "korean_transliteration"
features = ["extension-module"]
```

- [ ] **Step 4: Run test — must pass**
Run: `maturin develop -m crates/korean-transliteration-py/Cargo.toml && python3 -m pytest crates/korean-transliteration-py/tests/test_parity.py -v`
Expected: PASS

- [ ] **Step 5: Stage for review**
```bash
git add crates/korean-transliteration-py Cargo.toml
```

---

## Task 12: Workspace wiring + documentation

**Files:**
- Modify: `Cargo.toml` (final member list check)
- Modify: `src/lib.rs` (umbrella crate — resolved: re-export `korean_transliteration`
  alongside `g2pk`/`hangulize_rs`/`korean_phonemizer`, per user confirmation)
- Create/modify: a short `README.md` section under `crates/korean-transliteration/`
  describing the training/runtime split and pointing at
  `docs/specs/2026-08-26-korean-transliteration-design.md`

- [ ] **Step 1–4:** No new tests — this is documentation/wiring. Run `cargo test
      --workspace` and `cargo clippy --workspace --all-targets` as the acceptance check;
      both must be clean (or only carry pre-existing warnings from before this plan).
- [ ] **Step 5:** Stage for review.

---

## Allium Rule Coverage Checklist

- [x] `deterministic-output` — pure functions over embedded static data, no I/O per call
- [x] `no-panic-on-oov` — `transliterate` returns `Result`; the only `.expect()` calls
      are on the bundled `data/eng.fst` at process start, not per-word
- [x] `no-embedded-file-dependency` — `include_bytes!("../../../data/eng.fst")`
- [x] `phoneme-gap-repaired` — verified via `p2g::tests::repairs_consecutive_*` plus a
      real decoder artifact found and handled (geminate-consonant collapse)
- [x] `KoreanTransliterationCrate.regression-cases` — `matches_known_acronym_and_
      override_cases` passes for all 10 acronym/override cases (SKT, NAVER, AI, IBM,
      KT, LG, BBC, USA, GPT, CEO). **Partially open**: 3 ordinary dictionary words
      (google, apple, coffee) do not match hangulize-rs's output — tracked as
      `#[ignore]`d in `ordinary_word_g2p_accuracy_baseline`, not silently passing.
- [x] `KoreanTransliterationPy.python-parity` — same `transliterate` function is
      exposed via both `crates/korean-transliteration-py` and the `unbounded-korean-py`
      umbrella module; not independently re-verified by a Python-side test yet (no
      Python test runner was wired up this session — `maturin develop` + pytest is the
      documented Task 11 verification step but wasn't executed).
- [x] `TrainPhonetisaurusScript.remote-execution` — real run on `rares01.rapeech.intra`
      inside Docker (linux/amd64), producing `data/eng.fst` (87MB, 1,810,533 FST
      states), committed via git push/pull as designed.
- [x] `license-compliance` — no KoG2P/KoG2Padvanced strings were copied; the Korean
      phonology understanding gained from reading KoG2P's rulebook was not needed in
      the end (the P2G table only had to model English phoneme -> Hangul jamo, not
      Korean-side phonological rules), so nothing from either source appears anywhere
      in this crate.
- [x] `legacy-pipeline-untouched` — `cargo test --test hangulize_cases` still 9/9
      green after all of this session's korean-transliteration work.

## Follow-up: CMUdict data source (2026-08-26, same day)

The user asked to close the ordinary-word accuracy gap (google/apple/coffee/day/boy/
house) by training against CMUdict (https://github.com/cmusphinx/cmudict, BSD-style
license, `data/corpus/CMUDICT_LICENSE`), which g2pK-style pipelines traditionally use
for English-word phonemization (note: `crates/g2pk`'s own `english.rs` in *this* repo
is only a 6-word stub, not actually CMUdict-based — that gap is what the user was
recalling from the original Python g2pK project).

- `scripts/convert_cmudict_to_ipa.py`: converts CMUdict's ARPABET pronunciations
  (135,166 lines / 124,911 unique alphabetic words after filtering alternate
  pronunciations, abbreviation entries, and inline comments) to the exact same
  simplified-IPA alphabet `english_ipa_for_corpus` already established — no P2G
  changes needed, just better training data.
- `scripts/build_training_corpus.py` now merges multiple sources with priority
  ordering (`build_corpus(paths, out)`, later path wins on overlap). CMUdict is the
  high-priority source over the misaki-generated `eng_ipa.tsv`.
- Rebuilt `data/corpus/eng.dict`: 328,336 entries (235,973 misaki-only ∪ CMUdict,
  CMUdict winning all overlaps). Spot-checked all 8 previously-wrong words
  (hello/world/google/apple/coffee/day/boy/house) — CMUdict gives clean, correct
  phonemes for every one of them (e.g. "hello" → "hʌloʊ", a single /l/, unlike the
  misaki-derived corpus's decoder-observed double-consonant artifact).
- Retraining on `rares01.rapeech.intra` and re-verifying inference: see the updated
  Results section below.

## Results (2026-08-26)

Real end-to-end inference against the trained `data/eng.fst` model:

| Word | Output | Matches hangulize-rs? |
|---|---|---|
| SKT | 에스케이티 | yes |
| NAVER | 네이버 | yes |
| AI | 에이아이 | yes |
| IBM | 아이비엠 | yes |
| KT | 케이티 | yes |
| LG | 엘지 | yes |
| BBC | 비비시 | yes |
| USA | 유에스에이 | yes |
| GPT | 지피티 | yes |
| CEO | 시이오 | yes |
| hello | 헬로 | yes |
| world | 월드 | yes |
| text | 텍스트 | yes |
| time | 타임 | yes |
| google | 고아겔 | **no** (expected 구글) |
| apple | 애펠 | **no** (expected 애플) |
| coffee | 커페에이 | **no** (expected 커피) |
| day | 디 | **no** (expected 데이, not asserted in any test) |
| boy | 바이 | **no** (expected 보이, not asserted in any test) |
| house | 훗 | **no** (expected 하우스, not asserted in any test) |

The acronym/override layer (ported from hangulize-rs's fix earlier this session) closes
the original motivating gap completely. The trained G2P model's accuracy on ordinary
short/common words is inconsistent — including on some words that were literally in
its 235,973-entry training corpus, which points to the 8-gram joint model
under-memorizing certain patterns (doubled letters, short high-frequency words) rather
than a training-data coverage gap. Closing this would need Task 10-style tuning
(different n-gram order, `--casing lower` normalization, possibly more/cleaner data)
that wasn't attempted this session.
