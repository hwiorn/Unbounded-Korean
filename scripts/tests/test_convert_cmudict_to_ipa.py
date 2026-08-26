from convert_cmudict_to_ipa import arpabet_to_ipa, parse_cmudict_line, convert_cmudict


def test_arpabet_to_ipa_strips_stress_digits_and_maps_symbols():
    assert arpabet_to_ipa("HH") == "h"
    assert arpabet_to_ipa("AH1") == "ʌ"
    assert arpabet_to_ipa("OW1") == "oʊ"
    assert arpabet_to_ipa("L") == "l"


def test_arpabet_to_ipa_maps_unstressed_ah_to_schwa_not_wedge():
    # AH0 (no stress) is the reduced schwa, not the full /ʌ/ vowel of AH1/AH2 --
    # misaki's own English G2P already distinguishes them this way ("apple" ->
    # "æ p ə l" in data/corpus/eng_ipa.tsv), and P2G's collapse_syllabic_schwa_l
    # rule (apple -> 애플, not 애펄) keys specifically on 'ə', not 'ʌ'. Collapsing
    # both stress levels to 'ʌ' silently defeated that rule for every CMUdict word
    # with an unstressed AH0 before a syllabic L (apple, google, table, little...).
    assert arpabet_to_ipa("AH0") == "ə"
    assert arpabet_to_ipa("AH2") == "ʌ"


def test_parse_cmudict_line_converts_hello():
    # cmudict spells "hello" with two L's pronounced as one /l/ (single L phoneme,
    # unlike the Phonetisaurus decoder artifact this session found for misaki-derived
    # training data). Phonemes are space-separated, matching
    # hangulize-rs's english_ipa_for_corpus convention — required so Phonetisaurus's
    # (and later, korean-transliteration's own P2G bootstrap step's) default
    # whitespace lexicon-phoneme-separator tokenizes each symbol correctly instead of
    # treating the whole concatenated string as one atomic symbol.
    assert parse_cmudict_line("hello HH AH0 L OW1") == ("hello", "h ə l oʊ")


def test_parse_cmudict_line_skips_alternate_pronunciations():
    # Entries like "a(2)" are alternate pronunciations of a word already covered by
    # the bare "a" entry; keep only the primary entry per word.
    assert parse_cmudict_line("a(2) EY1") is None


def test_parse_cmudict_line_skips_non_alphabetic_words():
    assert parse_cmudict_line("'bout B AW1 T") is None
    assert parse_cmudict_line("a. EY1") is None


def test_parse_cmudict_line_strips_trailing_comment():
    # Some cmudict.dict entries carry an inline gloss comment, e.g. place/name origin.
    assert parse_cmudict_line("aalborg AO1 L B AO0 R G # place, danish") == (
        "aalborg",
        "ɔ l b ɔ ɹ ɡ",
    )


def test_convert_cmudict_produces_sorted_deduped_output(tmp_path):
    src = tmp_path / "cmudict.dict"
    src.write_text("world W ER1 L D\nhello HH AH0 L OW1\na(2) EY1\n'bout B AW1 T\n")
    out = tmp_path / "cmudict_ipa.tsv"
    convert_cmudict(src, out)
    assert out.read_text().splitlines() == [
        "hello\th ə l oʊ",
        "world\tw ɝ l d",
    ]
