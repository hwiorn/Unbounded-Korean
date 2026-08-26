/// Dutch, Italian, German, Spanish, Chinese, and Japanese, added after English as this
/// crate's first non-English pilot. Each trains from its own hangulize-rs `.hsl`
/// test-block entries only (no CMUdict/korean-go/muik equivalent exists for these
/// languages), so these corpora are far smaller than English's 345,747 words --
/// expect these tests to cover words the model has actually memorized, not held-out
/// generalization. Chinese and Japanese are logographic (see
/// examples/romanize_chi_corpus.rs and examples/romanize_jpn_corpus.rs): a known name
/// resolves through an exact-match dictionary, not the trained model.
#[test]
fn matches_known_words_in_each_alphabetic_language() {
    let cases = [
        ("nld", "Nicolaas", "니콜라스"),
        ("ita", "allegretto", "알레그레토"),
        ("deu", "Daniel", "다니엘"),
        ("spa", "braceo", "브라세오"),
    ];
    let mut failures = Vec::new();
    for (lang, word, expected) in cases {
        let actual = korean_transliteration::transliterate(lang, word).unwrap();
        if actual != expected {
            failures.push(format!("{lang} {word}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn matches_known_chinese_and_japanese_names_via_exact_dictionary() {
    let cases = [
        ("chi", "毛澤東", "마오쩌둥"),
        ("chi", "李彦宏", "리옌훙"),
        ("jpn", "木村拓哉", "기무라 다쿠야"),
        ("jpn", "新海誠", "신카이 마코토"),
    ];
    let mut failures = Vec::new();
    for (lang, word, expected) in cases {
        let actual = korean_transliteration::transliterate(lang, word).unwrap();
        if actual != expected {
            failures.push(format!("{lang} {word}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn falls_back_to_romanize_then_g2p_for_chinese_and_japanese_words_outside_the_dictionary() {
    // Neither word is in resources/{chi,jpn}_dictionary.tsv, so both must go through
    // pinyin/kana romanization and the trained model -- this only asserts the fallback
    // path runs end-to-end without erroring, not that it's linguistically perfect
    // (that's the same "generalization is imperfect on a tiny corpus" caveat as the
    // alphabetic languages above).
    assert!(korean_transliteration::transliterate("chi", "小龍女").is_ok());
    assert!(korean_transliteration::transliterate("jpn", "東京").is_ok());
}
