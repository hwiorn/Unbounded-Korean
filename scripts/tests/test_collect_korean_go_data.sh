#!/usr/bin/env bash
set -euo pipefail
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/korean-go.raw.txt" <<'EOF'
# 국립국어원 외래어 표기법 용례
kanaan	가나안
garnet	가넷
EOF

bash "$(dirname "$0")/../collect_korean_go_data.sh" "$tmp/korean-go.raw.txt" "$tmp/out.tsv"

expected=$'kanaan\t가나안\ngarnet\t가넷'
actual=$(cat "$tmp/out.tsv")
if [ "$actual" != "$expected" ]; then
  echo "FAIL: got:"
  echo "$actual"
  exit 1
fi
echo "PASS"
