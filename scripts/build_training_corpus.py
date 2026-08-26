"""Builds the Phonetisaurus training dictionary (word<TAB>phonemes, one per line,
sorted, deduplicated) from the bulk-generated (English word, IPA) corpus.

Only `data/corpus/eng_ipa.tsv` feeds this — see docs/plans/2026-08-26-
korean-transliteration-plan.md, Task 4, for why `hsl_seed.tsv` and `korean_go.tsv`
(word -> Hangul, not phonemes) are excluded here and used as the evaluation set
instead.
"""

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
