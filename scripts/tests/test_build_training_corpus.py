from build_training_corpus import build_corpus


def test_filters_noisy_passthrough_rows(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("hello\th ə l oʊ\nA\tA\n")  # "A\tA" is OOV passthrough noise
    out = tmp_path / "eng.dict"
    build_corpus(ipa, out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ"]


def test_rejects_duplicate_words(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("hello\th ə l oʊ\nhello\th ə l oʊ\n")
    out = tmp_path / "eng.dict"
    build_corpus(ipa, out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ"]


def test_sorts_output_by_word(tmp_path):
    ipa = tmp_path / "eng_ipa.tsv"
    ipa.write_text("world\tw ɜ l d\nhello\th ə l oʊ\n")
    out = tmp_path / "eng.dict"
    build_corpus(ipa, out)
    assert out.read_text().splitlines() == ["hello\th ə l oʊ", "world\tw ɜ l d"]
