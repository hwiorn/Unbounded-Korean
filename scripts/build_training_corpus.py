"""Builds the Phonetisaurus training dictionary (word<TAB>phonemes, one per line,
sorted, deduplicated) from one or more (English word, IPA) source files.

Sources are merged in priority order: later paths override earlier ones for the same
word. `data/corpus/eng_ipa.tsv` (misaki-generated, full /usr/share/dict/words
coverage) is the low-priority base; `data/corpus/cmudict_ipa.tsv` (CMUdict, BSD
license, professionally curated) is the high-priority override for the ~125k words it
covers — CMUdict doesn't have this session's Phonetisaurus-decoder-observed quality
issues (e.g. a misaki/OOV-decode artifact double-counting a consonant), so it wins
where both sources have an entry.

`hsl_seed.tsv` and `korean_go.tsv` (word -> Hangul, not phonemes) are excluded here and
used as the evaluation set instead — see docs/plans/2026-08-26-
korean-transliteration-plan.md, Task 4.
"""

import sys
from pathlib import Path


def build_corpus(ipa_paths: list[Path], out_path: Path) -> None:
    seen: dict[str, str] = {}
    for ipa_path in ipa_paths:
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
    *sources, output = sys.argv[1:]
    build_corpus([Path(p) for p in sources], Path(output))
