#!/usr/bin/env bash
# Normalizes a muik/transliteration data/source/*.txt file (leading '#' comment
# lines + tab-delimited english<TAB>hangul pairs) into a clean TSV with only the
# pairs. Does not fetch the file itself — see fetch_korean_go_data.sh for that.
#
# Usage: collect_korean_go_data.sh <input-raw-tsv> <output-tsv>
set -euo pipefail
in_file="$1"
out_file="$2"
grep -v '^#' "$in_file" | grep -v '^[[:space:]]*$' > "$out_file"
