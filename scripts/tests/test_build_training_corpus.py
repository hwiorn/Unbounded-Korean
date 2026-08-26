from build_training_corpus import build_corpus


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
