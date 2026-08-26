"""Builds the Phonetisaurus training dictionary (word<TAB>phonemes, one per line,
sorted, deduplicated) from one or more (English word, IPA) source files.

Sources are merged in priority order: later paths override earlier ones for the same
word. From lowest to highest priority:

  1. `eng_ipa.tsv` — misaki-generated, full /usr/share/dict/words coverage, but a
     guess (no human ever verified these pronunciations).
  2. `hsl_eng_ipa.tsv` — hangulize-rs's eng.hsl test cases, run through this
     project's own Korean G2P (see examples/hangul_answer_to_ipa_corpus.rs): a real,
     human-verified (word, Hangul) pair converted back into phonemes, not guessed.
  3. `muik_other_ipa.tsv` — muik/transliteration's remaining data/source files
     (cities, dictionary, doosan, self, suggests, wiktionary), same derivation.
  4. `cmudict_ipa.tsv` — CMUdict (BSD-style license, professionally curated ARPABET
     dictionary): no Hangul-derivation step at all, so no g2pk-introduced noise. But
     CMUdict lists one ARPABET pronunciation as "primary" per word with no guarantee
     it matches Korean loanword convention — e.g. "mileage"'s primary entry uses the
     unstressed schwa AH0 (as does every other CMUdict "-age" word: cottage, village,
     package, message, storage), which renders as 마일러지, not the 마일리지 korean-go
     actually uses. So CMUdict wins over the guessed/lower-provenance sources above,
     but not over an official answer below.
  5. `korean_go_ipa.tsv` — muik/transliteration's korean-go.txt (국립국어원-sourced):
     an official Korean government loanword spelling, not a pronunciation this
     project selected among alternatives, so it's the trump card wherever it has an
     entry.
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
