from build_training_corpus import build_corpus, double_intervocalic_l


def test_filters_noisy_passthrough_rows(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("hello\th ə l oʊ\nA\tA\n")  # "A\tA" is OOV passthrough noise
    out = tmp_path / "eng.dict"
    build_corpus([ipa], out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ"]


def test_rejects_duplicate_words(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("hello\th ə l oʊ\nhello\th ə l oʊ\n")
    out = tmp_path / "eng.dict"
    build_corpus([ipa], out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ"]


def test_sorts_output_by_word(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("world\tw ɜ l d\nhello\th ə l oʊ\n")
    out = tmp_path / "eng.dict"
    build_corpus([ipa], out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ", "world\tw ɜ l d"]


def test_later_source_overrides_earlier_for_the_same_word(tmp_path):
    # Lower-priority source first (e.g. misaki-generated), higher-priority source
    # last (e.g. CMUdict) — the higher-priority pronunciation wins on overlap, and
    # words unique to either source are still kept.
    low_priority = tmp_path / "eng_ipa.tsv"
    low_priority.write_text("hello\thʌlloʊ\nonly_in_low\tx\n")
    high_priority = tmp_path / "cmudict_ipa.tsv"
    high_priority.write_text("hello\thʌloʊ\nonly_in_high\ty\n")
    out = tmp_path / "eng.dict"
    build_corpus([low_priority, high_priority], out)
    assert out.read_text().splitlines() == [
        "hello\thʌloʊ",
        "only_in_high\ty",
        "only_in_low\tx",
    ]


def test_double_intervocalic_l_doubles_a_lone_l_between_two_vowels():
    # "hello" h ə l oʊ -- CMUdict/misaki give the real single English /l/ phoneme,
    # but Korean loanword orthography doubles an intervocalic /l/ into ㄹㄹ (헬로,
    # not 헤로) -- see crates/korean-transliteration/src/p2g.rs's
    # collapse_geminate_consonants doc comment for the P2G side of this rule.
    assert double_intervocalic_l("h ə l oʊ") == "h ə l l oʊ"


def test_double_intervocalic_l_leaves_the_rhotic_untouched():
    # "hero" h ɪ ɹ oʊ -- American /ɹ/ does NOT double intervocalically in Korean
    # loanword orthography (히어로, not 힐러로 or 히얼로): only /l/ does.
    assert double_intervocalic_l("h ɪ ɹ oʊ") == "h ɪ ɹ oʊ"


def test_double_intervocalic_l_leaves_a_word_boundary_l_untouched():
    # Word-initial ("lion") and word-final ("well") /l/ aren't intervocalic, so
    # neither doubles -- only becomes an onset or coda respectively.
    assert double_intervocalic_l("l aɪ ə n") == "l aɪ ə n"
    assert double_intervocalic_l("w ɛ l") == "w ɛ l"


def test_raw_ipa_sources_get_l_doubling_but_hangul_derived_sources_do_not(tmp_path):
    # eng_ipa.tsv and cmudict_ipa.tsv carry a real, undoubled English /l/ that needs
    # this transform. hsl_eng_ipa.tsv/muik_other_ipa.tsv/korean_go_ipa.tsv are
    # already derived from the correct Hangul answer's own spelling (see
    # hangul_answer_to_ipa_corpus.rs), so their intervocalic 'l' count is already
    # correct and must not be doubled again.
    raw_ipa = tmp_path / "cmudict_ipa.tsv"
    raw_ipa.write_text("hello\th ə l oʊ\n")
    hangul_derived = tmp_path / "korean_go_ipa.tsv"
    hangul_derived.write_text("neuron\tn j u l ʌ n\n")
    out = tmp_path / "eng.dict"
    build_corpus([raw_ipa, hangul_derived], out, raw_ipa_sources=frozenset([raw_ipa]))
    assert out.read_text().splitlines() == [
        "hello\th ə l l oʊ",
        "neuron\tn j u l ʌ n",
    ]
