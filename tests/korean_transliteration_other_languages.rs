/// Every hangulize-rs language besides English: Dutch, Italian, German, and Spanish
/// (this crate's first non-English pilot), Chinese and Japanese (logographic --
/// romanized through pinyin/kana first, see examples/romanize_chi_corpus.rs and
/// examples/romanize_jpn_corpus.rs -- with an exact-match dictionary fast path for
/// known names), and 31 more (matches_one_known_word_in_each_remaining_language,
/// below) covering every remaining `.hsl` spec. Each trains from its own hsl
/// test-block entries only (no CMUdict/korean-go/muik equivalent exists for any of
/// these languages), so these corpora are far smaller than English's 345,747 words --
/// expect these tests to cover words the model has actually memorized, not held-out
/// generalization.
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

/// The remaining 31 hangulize-rs languages, each trained purely on its own hsl
/// test-block entries (14 to 351 of them -- see the corresponding commit for exact
/// sizes). One verified-correct in-corpus word per language, confirmed by actually
/// running every single-word hsl entry through transliterate() for each language: the
/// overall exact-match rate lands around 60% (word count varies a lot by language, from
/// vie's 1/6 to lav's 16/20), consistent with the same "tiny corpus, imperfect
/// generalization" pattern already documented for nld/ita/deu/spa -- this test locks in
/// one word this crate is verified to get right per language, not a quality claim for
/// words outside each corpus.
#[test]
fn matches_one_known_word_in_each_remaining_language() {
    let cases = [
        ("aze", "jurnal", "주르날"),
        ("bel", "Полацк", "폴라츠크"),
        ("bul", "София", "소피야"),
        ("cat", "Elx", "엘시"),
        ("ces", "kachna", "카흐나"),
        ("cym", "Calennig", "칼레니그"),
        ("ell", "προϋπολογίζω", "프로이폴로이조"),
        ("epo", "Jes", "예스"),
        ("est", "Kalevipoeg", "칼레비포에그"),
        ("fin", "Turku", "투르쿠"),
        ("grc", "Κυβέλη", "키벨레"),
        ("hbs", "jastuk", "야스투크"),
        ("hun", "csomag", "초머그"),
        ("isl", "Þjórsá", "시오르사우"),
        ("kat", "ბურჯანაძე", "부르자나제"),
        ("lat", "Iuno", "유노"),
        ("lav", "Daugava", "다우가바"),
        ("lit", "Panevėžys", "파네베지스"),
        ("mkd", "Кичево", "키체보"),
        ("pol", "dywan", "디반"),
        ("por", "Montes", "몬트스"),
        ("ron", "este", "예스테"),
        ("rus", "Аввакум", "아바쿰"),
        ("slk", "Nitra", "니트라"),
        ("slv", "Trbovlje", "트르보울리에"),
        ("sqi", "Ulpiana", "울피아나"),
        ("swe", "detalj", "데탈리"),
        ("tur", "İzmir", "이즈미르"),
        ("ukr", "кобзар", "코브자르"),
        ("vie", "yên", "옌"),
        ("wlm", "Olwen", "올웬"),
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
