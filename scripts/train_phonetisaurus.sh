#!/usr/bin/env bash
# Trains a Phonetisaurus G2P model for one source language and writes data/<lang>.fst.
#
# Must run against a Linux x86_64 host: the from-source build and the rhasspy PyPI
# wheel fallback both target that platform (see docs/specs/2026-08-26-
# korean-transliteration-design.md for why this can't run natively on macOS arm64).
# Delivers code to the remote host over git (push/clone/pull), not scp, per this
# session's plan.
#
# Usage: train_phonetisaurus.sh <lang> [--dry-run]
# TRAIN_NGRAM_ORDER=<n> overrides the joint n-gram order. Default is 8, matching the
# high-level `phonetisaurus train` CLI's own default. An order 4/5/6/8 comparison on
# eng.dict (2026-08-26, before this pipeline's P2G rendering bugs were fixed) found
# order 6 tracked order 8 closely enough to prefer the smaller model (21.3MB vs
# 37.8MB gzipped). Re-measured after fixing those P2G bugs (2026-08-27): with correct
# rendering, order 8 recovers more of the model's own correct training data at decode
# time than order 6 does (korean_go.tsv benchmark 78.6% -> 80.3%), so the extra size
# is worth it again for English's large corpus. See docs/plans/2026-08-26-
# korean-transliteration-plan.md for the original comparison.
set -euo pipefail

REMOTE_HOST="${TRAIN_REMOTE_HOST:-gglee@rares01.rapeech.intra}"
REMOTE_REPO_URL="${TRAIN_REMOTE_REPO_URL:-git@github.com:hwiorn/Unbounded-Korean.git}"
NGRAM_ORDER="${TRAIN_NGRAM_ORDER:-8}"
LANG_CODE="${1:?usage: train_phonetisaurus.sh <lang> [--dry-run]}"
MODE="${2:-}"

if [ "$MODE" = "--dry-run" ]; then
  echo "[dry-run] would push current branch to origin"
  echo "[dry-run] would ssh ${REMOTE_HOST} and clone/pull ${REMOTE_REPO_URL}"
  echo "[dry-run] would docker build -f docker/phonetisaurus-train.Dockerfile"
  echo "[dry-run] would align+estimate+convert data/corpus/${LANG_CODE}.dict -> data/${LANG_CODE}.fst (seq2_max=3, ngram_order=${NGRAM_ORDER})"
  echo "[dry-run] would commit+push data/${LANG_CODE}.fst from the remote host"
  echo "[dry-run] would git pull the result back locally"
  exit 0
fi

CORPUS="data/corpus/${LANG_CODE}.dict"
if [ ! -f "$CORPUS" ]; then
  echo "error: training corpus not found: $CORPUS" >&2
  exit 1
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
echo "Pushing ${BRANCH} to origin..."
git push origin "$BRANCH"

echo "Running training on ${REMOTE_HOST} (ngram_order=${NGRAM_ORDER})..."
ssh "$REMOTE_HOST" bash -s "$BRANCH" "$LANG_CODE" "$REMOTE_REPO_URL" "$NGRAM_ORDER" <<'REMOTE_SCRIPT'
set -euo pipefail
BRANCH="$1"
LANG_CODE="$2"
REPO_URL="$3"
NGRAM_ORDER="$4"
WORKDIR="$HOME/korean-transliteration-train"

if [ ! -d "$WORKDIR" ]; then
  git clone "$REPO_URL" "$WORKDIR"
fi
cd "$WORKDIR"
git fetch origin "$BRANCH"
git checkout "$BRANCH"
git pull origin "$BRANCH"

docker build --platform linux/amd64 -f docker/phonetisaurus-train.Dockerfile -t phonetisaurus-train .

# The high-level `phonetisaurus train` CLI hardcodes seq1_max=2/seq2_max=2 (and
# ngram_order=8) with no flag to change them. seq2_max=2 caps how many output
# phonemes a single input grapheme (or 2-grapheme chunk) can absorb during
# alignment -- too small for short, phoneme-dense entries like a Korean
# acronym letter name ("SKT" -> e s k e i t i needs 3 phonemes for the single
# letter K alone), which then fail alignment outright and are silently
# dropped from training. Call the same three lower-level binaries the wrapper
# itself shells out to (confirmed via --debug), with seq2_max raised to 3 --
# enough for every letter name in this corpus except lone "W" (더블유, 6-7
# phonemes) -- while leaving every other default (seq1_max, ngram_order,
# seq1_del/seq2_del) exactly as the wrapper sets them.
docker run --rm --platform linux/amd64 -v "$WORKDIR/data:/work/data" phonetisaurus-train \
  bash -c '
    set -euo pipefail
    BIN=/usr/local/lib/python3.11/dist-packages/phonetisaurus/bin/x86_64
    export LD_LIBRARY_PATH="/usr/local/lib/python3.11/dist-packages/phonetisaurus/lib/x86_64:${LD_LIBRARY_PATH:-}"
    mkdir -p /work/data/train
    "$BIN/phonetisaurus-align" --input="/work/data/corpus/'"${LANG_CODE}"'.dict" \
      --ofile=/work/data/train/model.corpus \
      --seq1_del=false --seq2_del=true --seq1_max=2 --seq2_max=3 --grow=false
    "$BIN/estimate-ngram" -o '"${NGRAM_ORDER}"' -t /work/data/train/model.corpus -wl /work/data/train/model.arpa
    "$BIN/phonetisaurus-arpa2wfst" --lm=/work/data/train/model.arpa --ofile="/work/data/'"${LANG_CODE}"'.fst"
  '

# The raw .fst can exceed GitHub's 100MB hard limit (an 8-gram joint model over a
# 300k+-entry corpus is well over that). It compresses to ~30% of its size (a lot of
# structural redundancy in the FST binary format), so ship it as .fst.gz and
# decompress at load time in the Rust crate instead of the raw file.
gzip -kf "data/${LANG_CODE}.fst"
rm -f "data/${LANG_CODE}.fst"

git add "data/${LANG_CODE}.fst.gz"
git commit -m "data: train ${LANG_CODE}.fst.gz on $(hostname)"
git push origin "$BRANCH"
REMOTE_SCRIPT

echo "Pulling trained model back..."
git pull origin "$BRANCH"

if [ -f "data/${LANG_CODE}.fst.gz" ]; then
  echo "Trained model available at data/${LANG_CODE}.fst.gz"
else
  echo "error: expected data/${LANG_CODE}.fst.gz after pull, but it is missing" >&2
  exit 1
fi
