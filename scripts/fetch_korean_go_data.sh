#!/usr/bin/env bash
# One-time fetch of muik/transliteration's government-sourced (국립국어원)
# English->Korean data file. Network call to a public GitHub raw URL.
#
# Usage: fetch_korean_go_data.sh <output-raw-path>
set -euo pipefail
out_file="$1"
url="https://raw.githubusercontent.com/muik/transliteration/master/data/source/korean-go.txt"
curl -fsSL "$url" -o "$out_file"
