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
set -euo pipefail

REMOTE_HOST="${TRAIN_REMOTE_HOST:-gglee@rares01.rapeech.intra}"
REMOTE_REPO_URL="${TRAIN_REMOTE_REPO_URL:-git@github.com:hwiorn/Unbounded-Korean.git}"
LANG_CODE="${1:?usage: train_phonetisaurus.sh <lang> [--dry-run]}"
MODE="${2:-}"

if [ "$MODE" = "--dry-run" ]; then
  echo "[dry-run] would push current branch to origin"
  echo "[dry-run] would ssh ${REMOTE_HOST} and clone/pull ${REMOTE_REPO_URL}"
  echo "[dry-run] would docker build -f docker/phonetisaurus-train.Dockerfile"
  echo "[dry-run] would train data/corpus/${LANG_CODE}.dict -> data/${LANG_CODE}.fst"
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

echo "Running training on ${REMOTE_HOST}..."
ssh "$REMOTE_HOST" bash -s "$BRANCH" "$LANG_CODE" "$REMOTE_REPO_URL" <<'REMOTE_SCRIPT'
set -euo pipefail
BRANCH="$1"
LANG_CODE="$2"
REPO_URL="$3"
WORKDIR="$HOME/korean-transliteration-train"

if [ ! -d "$WORKDIR" ]; then
  git clone "$REPO_URL" "$WORKDIR"
fi
cd "$WORKDIR"
git fetch origin "$BRANCH"
git checkout "$BRANCH"
git pull origin "$BRANCH"

docker build --platform linux/amd64 -f docker/phonetisaurus-train.Dockerfile -t phonetisaurus-train .
docker run --rm --platform linux/amd64 -v "$WORKDIR/data:/work/data" phonetisaurus-train \
  phonetisaurus train --model "/work/data/${LANG_CODE}.fst" "/work/data/corpus/${LANG_CODE}.dict"

git add "data/${LANG_CODE}.fst"
git commit -m "data: train ${LANG_CODE}.fst on $(hostname)"
git push origin "$BRANCH"
REMOTE_SCRIPT

echo "Pulling trained model back..."
git pull origin "$BRANCH"

if [ -f "data/${LANG_CODE}.fst" ]; then
  echo "Trained model available at data/${LANG_CODE}.fst"
else
  echo "error: expected data/${LANG_CODE}.fst after pull, but it is missing" >&2
  exit 1
fi
