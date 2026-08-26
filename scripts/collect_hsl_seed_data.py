"""Extract (word, hangul) pairs from hangulize-rs's per-language .hsl `test:` blocks.

These pairs are a supplementary seed corpus (a few dozen to a few hundred pairs per
language) for training a Phonetisaurus G2P model — not a primary data source. See
docs/specs/2026-08-26-korean-transliteration-design.md.
"""

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
