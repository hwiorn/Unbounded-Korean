"""Converts CMUdict (https://github.com/cmusphinx/cmudict, BSD-style license) ARPABET
pronunciations into the same simplified-IPA phoneme alphabet used everywhere else in
this pipeline (crates/hangulize-rs's `english_ipa_for_corpus`, and
crates/korean-transliteration's P2G table) — so this becomes a drop-in, higher-quality
replacement/supplement for the misaki-generated (word, IPA) corpus, with no changes
needed to the P2G decoder.
"""

import re
import sys
from pathlib import Path

# Standard ARPABET -> IPA correspondence (stress digits 0/1/2 stripped before lookup;
# this pipeline's phoneme alphabet doesn't track stress, matching
# `simplify_phoneme_string` in hangulize-rs).
_VOWELS = {
    "AA": "ɑ",
    "AE": "æ",
    "AH": "ʌ",
    "AO": "ɔ",
    "AW": "aʊ",
    "AY": "aɪ",
    "EH": "ɛ",
    "ER": "ɝ",
    "EY": "eɪ",
    "IH": "ɪ",
    "IY": "i",
    "OW": "oʊ",
    "OY": "ɔɪ",
    "UH": "ʊ",
    "UW": "u",
}
_CONSONANTS = {
    "B": "b",
    "CH": "tʃ",
    "D": "d",
    "DH": "ð",
    "F": "f",
    "G": "ɡ",
    "HH": "h",
    "JH": "dʒ",
    "K": "k",
    "L": "l",
    "M": "m",
    "N": "n",
    "NG": "ŋ",
    "P": "p",
    "R": "ɹ",
    "S": "s",
    "SH": "ʃ",
    "T": "t",
    "TH": "θ",
    "V": "v",
    "W": "w",
    "Y": "j",
    "Z": "z",
    "ZH": "ʒ",
}

_WORD_RE = re.compile(r"[a-z][a-z']*")


def arpabet_to_ipa(token: str) -> str:
    base = token.rstrip("012")
    if base in _VOWELS:
        return _VOWELS[base]
    if base in _CONSONANTS:
        return _CONSONANTS[base]
    raise KeyError(f"unknown ARPABET symbol: {token!r}")


def parse_cmudict_line(line: str) -> tuple[str, str] | None:
    line = line.split("#", 1)[0].strip()
    if not line:
        return None
    parts = line.split()
    word, phonemes = parts[0], parts[1:]
    if not _WORD_RE.fullmatch(word):
        return None  # "a(2)" (alt. pronunciation), "a." (abbreviation), "'bout" (elided
        # informal contraction) — mid-word apostrophes ("it's") are still allowed
    return word, " ".join(arpabet_to_ipa(p) for p in phonemes)


def convert_cmudict(src_path: Path, out_path: Path) -> None:
    seen: dict[str, str] = {}
    for line in src_path.read_text(encoding="utf-8").splitlines():
        result = parse_cmudict_line(line)
        if result is None:
            continue
        word, ipa = result
        seen[word] = ipa
    with open(out_path, "w", encoding="utf-8") as f:
        for word in sorted(seen):
            f.write(f"{word}\t{seen[word]}\n")


if __name__ == "__main__":
    convert_cmudict(Path(sys.argv[1]), Path(sys.argv[2]))
