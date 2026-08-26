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

# Mirrors crates/korean-transliteration/src/p2g.rs's `unit_for` vowel arms -- the
# vowel side of the "is this 'l' intervocalic" check in `double_intervocalic_l`
# below must agree with what P2G itself treats as a vowel.
_VOWELS = {
    "æ", "ɛ", "ə", "e", "ᵻ", "ɜ", "ʌ", "ɔ", "ɚ", "ɝ", "ɑ", "a", "i", "ɪ", "u", "ʊ",
    "o", "oʊ", "eɪ", "aɪ", "aʊ", "ɔɪ",
}

# Filenames whose phoneme column is real, undoubled English IPA (misaki/CMUdict),
# not Hangul-answer-derived -- see `double_intervocalic_l`'s doc comment.
_RAW_IPA_FILENAMES = {"eng_ipa.tsv", "cmudict_ipa.tsv"}


def double_intervocalic_l(phonemes: str) -> str:
    """Korean loanword orthography doubles an English intervocalic /l/ into ㄹㄹ
    (coda + onset), e.g. "hello" 헬로, but leaves the rhotic /ɹ/ single, e.g. "hero"
    히어로 (see crates/korean-transliteration/src/p2g.rs's
    collapse_geminate_consonants for the P2G side of this rule). CMUdict and misaki
    give the real, single English /l/ phoneme -- this applies the doubling those
    sources don't encode. Hangul-answer-derived sources (hsl_eng_ipa.tsv,
    muik_other_ipa.tsv, korean_go_ipa.tsv) already encode the correct count via the
    real answer's own spelling and must not be passed through this function.
    """
    tokens = phonemes.split(" ")
    out: list[str] = []
    for i, token in enumerate(tokens):
        out.append(token)
        if (
            token == "l"
            and i > 0
            and i + 1 < len(tokens)
            and tokens[i - 1] in _VOWELS
            and tokens[i + 1] in _VOWELS
        ):
            out.append(token)
    return " ".join(out)


def build_corpus(
    ipa_paths: list[Path],
    out_path: Path,
    raw_ipa_sources: frozenset[Path] = frozenset(),
) -> None:
    seen: dict[str, str] = {}
    for ipa_path in ipa_paths:
        needs_l_doubling = ipa_path in raw_ipa_sources
        for line in ipa_path.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            word, phonemes = line.split("\t", 1)
            if word == phonemes:
                continue  # OOV passthrough noise: no real phonemization happened
            if needs_l_doubling:
                phonemes = double_intervocalic_l(phonemes)
            seen[word] = phonemes
    with open(out_path, "w", encoding="utf-8") as f:
        for word in sorted(seen):
            f.write(f"{word}\t{seen[word]}\n")


if __name__ == "__main__":
    *sources, output = sys.argv[1:]
    paths = [Path(p) for p in sources]
    raw_ipa_sources = frozenset(p for p in paths if p.name in _RAW_IPA_FILENAMES)
    build_corpus(paths, Path(output), raw_ipa_sources)
