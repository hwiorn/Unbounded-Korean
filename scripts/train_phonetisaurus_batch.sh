#!/usr/bin/env bash
# Batch variant of train_phonetisaurus.sh: trains multiple languages in ONE remote
# session (single push/clone/docker-build/commit/push) instead of paying that
# per-language round-trip overhead once per language -- worthwhile once there are
# many small corpora to (re)train at once, e.g. after a P2G/reverse.rs fix that
# needs every non-English language's tiny hangulize-rs-derived corpus retrained.
#
# Usage: train_phonetisaurus_batch.sh <lang1> <lang2> ...
# TRAIN_NGRAM_ORDER=<n> overrides the joint n-gram order (default 8, see
# train_phonetisaurus.sh's own doc comment for why).
set -euo pipefail

REMOTE_HOST="${TRAIN_REMOTE_HOST:-gglee@rares01.rapeech.intra}"
REMOTE_REPO_URL="${TRAIN_REMOTE_REPO_URL:-git@github.com:hwiorn/Unbounded-Korean.git}"
NGRAM_ORDER="${TRAIN_NGRAM_ORDER:-8}"

if [ "$#" -eq 0 ]; then
  echo "usage: train_phonetisaurus_batch.sh <lang1> <lang2> ..." >&2
  exit 1
fi

for lang in "$@"; do
  if [ ! -f "data/corpus/${lang}.dict" ]; then
    echo "error: training corpus not found: data/corpus/${lang}.dict" >&2
    exit 1
  fi
done

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
echo "Pushing ${BRANCH} to origin..."
git push origin "$BRANCH"

echo "Running batch training on ${REMOTE_HOST} (ngram_order=${NGRAM_ORDER}) for: $*"
ssh "$REMOTE_HOST" bash -s "$BRANCH" "$REMOTE_REPO_URL" "$NGRAM_ORDER" "$@" <<'REMOTE_SCRIPT'
set -euo pipefail
BRANCH="$1"; shift
REPO_URL="$1"; shift
NGRAM_ORDER="$1"; shift
LANGS=("$@")
WORKDIR="$HOME/korean-transliteration-train"

if [ ! -d "$WORKDIR" ]; then
  git clone "$REPO_URL" "$WORKDIR"
fi
cd "$WORKDIR"
git fetch origin "$BRANCH"
git checkout "$BRANCH"
git pull origin "$BRANCH"

docker build --platform linux/amd64 -f docker/phonetisaurus-train.Dockerfile -t phonetisaurus-train .

TRAINED=()
SKIPPED=()
FAILED=()
for LANG_CODE in "${LANGS[@]}"; do
  LINES=$(wc -l < "data/corpus/${LANG_CODE}.dict")
  if [ "$LINES" -lt 5 ]; then
    echo "=== skipping ${LANG_CODE}: only ${LINES} training entries, too few to estimate any n-gram model (leaving its existing .fst.gz untouched) ==="
    SKIPPED+=("${LANG_CODE}(${LINES} entries)")
    continue
  fi
  # estimate-ngram needs enough distinct n-grams to fit its discounting
  # parameters at the requested order -- a handful of these hangulize-rs-derived
  # corpora are tiny (tens of entries) and segfault outright at the default
  # order 8, so cap it to the corpus size for those specifically rather than
  # skipping them entirely.
  ORDER="$NGRAM_ORDER"
  if [ "$LINES" -lt "$NGRAM_ORDER" ]; then
    ORDER=$((LINES - 1))
  fi
  echo "=== training ${LANG_CODE} (${LINES} entries, ngram_order=${ORDER}) ==="
  # A raw line count doesn't predict alignment success -- e.g. Chinese's
  # multi-character-per-name entries mostly fail phonetisaurus-align outright,
  # leaving too few ALIGNED pairs for estimate-ngram regardless of how many
  # raw .dict lines there were, which segfaults it the same way a too-small
  # LINES would. Isolate each language's training in its own subshell instead
  # of predicting this in advance, so one language's crash can't take the
  # whole batch down with it (set -e is intentionally suspended only here).
  set +e
  docker run --rm --platform linux/amd64 -v "$WORKDIR/data:/work/data" phonetisaurus-train \
    bash -c '
      set -euo pipefail
      BIN=/usr/local/lib/python3.11/dist-packages/phonetisaurus/bin/x86_64
      export LD_LIBRARY_PATH="/usr/local/lib/python3.11/dist-packages/phonetisaurus/lib/x86_64:${LD_LIBRARY_PATH:-}"
      mkdir -p /work/data/train
      "$BIN/phonetisaurus-align" --input="/work/data/corpus/'"${LANG_CODE}"'.dict" \
        --ofile="/work/data/train/'"${LANG_CODE}"'.model.corpus" \
        --seq1_del=false --seq2_del=true --seq1_max=2 --seq2_max=3 --grow=false
      "$BIN/estimate-ngram" -o '"${ORDER}"' -t "/work/data/train/'"${LANG_CODE}"'.model.corpus" -wl "/work/data/train/'"${LANG_CODE}"'.model.arpa"
      "$BIN/phonetisaurus-arpa2wfst" --lm="/work/data/train/'"${LANG_CODE}"'.model.arpa" --ofile="/work/data/'"${LANG_CODE}"'.fst"
    '
  STATUS=$?
  set -e
  if [ "$STATUS" -ne 0 ]; then
    echo "=== ${LANG_CODE} training failed (exit ${STATUS}), leaving its existing .fst.gz untouched ==="
    FAILED+=("${LANG_CODE}(exit ${STATUS})")
    continue
  fi
  gzip -kf "data/${LANG_CODE}.fst"
  rm -f "data/${LANG_CODE}.fst"
  git add "data/${LANG_CODE}.fst.gz"
  TRAINED+=("$LANG_CODE")
done
echo "trained: ${TRAINED[*]:-none}"
echo "skipped (too little data): ${SKIPPED[*]:-none}"
echo "failed (training error): ${FAILED[*]:-none}"

if [ "${#TRAINED[@]}" -gt 0 ]; then
  git commit -m "data: batch-train $(IFS=,; echo "${TRAINED[*]}") .fst.gz on $(hostname)"
  git push origin "$BRANCH"
else
  echo "nothing trained -- skipping commit"
fi
REMOTE_SCRIPT

echo "Pulling trained models back..."
git pull origin "$BRANCH"

missing=0
for lang in "$@"; do
  if [ ! -f "data/${lang}.fst.gz" ]; then
    echo "error: expected data/${lang}.fst.gz after pull, but it is missing" >&2
    missing=1
  fi
done
if [ "$missing" -eq 0 ]; then
  echo "All models trained: $*"
else
  exit 1
fi
