/// These are the cases that motivated this crate: SKT/NAVER-style acronyms and brand
/// names that hangulize-rs's misaki-based pipeline used to garble (see this session's
/// earlier hangulize-rs fix). Handled entirely by the trained Phonetisaurus model plus
/// P2G -- no hardcoded per-word table -- via training data derived from hsl.eng's test
/// cases and muik/transliteration's korean-go.txt and other sources, converted to
/// phonemes through this project's own Korean G2P (see
/// examples/hangul_answer_to_ipa_corpus.rs).
#[test]
fn matches_known_acronym_cases() {
    let cases = [
        ("SKT", "에스케이티"),
        ("NAVER", "네이버"),
        ("AI", "에이아이"),
        ("IBM", "아이비엠"),
        ("KT", "케이티"),
        ("LG", "엘지"),
        ("BBC", "비비시"),
        ("USA", "유에스에이"),
        ("GPT", "지피티"),
        ("CEO", "시이오"),
    ];
    let mut failures = Vec::new();
    for (word, expected) in cases {
        let actual = korean_transliteration::transliterate("eng", word).unwrap();
        if actual != expected {
            failures.push(format!("{word}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Ordinary dictionary words go through the trained Phonetisaurus model, not any
/// per-word table.
#[test]
fn matches_ordinary_word_cases() {
    let cases = [
        ("hello", "헬로"),
        ("world", "월드"),
        ("google", "구글"),
        ("apple", "애플"),
        ("coffee", "커피"),
        ("text", "텍스트"),
    ];
    let mut failures = Vec::new();
    for (word, expected) in cases {
        let actual = korean_transliteration::transliterate("eng", word).unwrap();
        if actual != expected {
            failures.push(format!("{word}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// "eng" and "eng-us" are trained on deliberately different corpora, not the
/// same data with different priority: "eng" merges only Hangul-answer-derived
/// sources (misaki-seeded, then hsl_eng/muik/korean_go override where they
/// have an entry) -- Korea's own established loanword convention. "eng-us" is
/// cmudict alone -- real American pronunciation, with no Korean convention
/// involved. They can legitimately disagree ("mileage" 마일리지 vs 마일러지,
/// since cmudict's primary pronunciation uses the unstressed schwa AH0, not
/// the "mileage(2)" alternate IH0 pronunciation the established 마일리지
/// spelling actually reflects -- crates/g2pk's own cmudict-based port
/// documents this exact same limitation and reaches the same 마일러지
/// answer) -- neither is "wrong", they answer different questions. An acronym
/// like "SKT" is a Korean-orthography convention (spelling out each letter's
/// Korean name) with no equivalent in real English pronunciation, so
/// "eng-us" has nothing to decode it from at all.
#[test]
fn eng_and_eng_us_answer_different_questions_for_the_same_word() {
    assert_eq!(
        korean_transliteration::transliterate("eng", "mileage").unwrap(),
        "마일리지"
    );
    assert_eq!(
        korean_transliteration::transliterate("eng-us", "mileage").unwrap(),
        "마일러지"
    );
    assert!(korean_transliteration::transliterate("eng-us", "SKT").is_err());
}

/// A word with no entry in any Hangul-answer-derived source (hsl_eng/muik/
/// korean_go) trained "eng" on nothing but misaki's guess -- "eng" now
/// substitutes "eng-us"'s cmudict-based decode for exactly these words
/// instead, at the application level (mixing cmudict into eng.dict's own
/// training corpus measurably hurt accuracy even for words the merge never
/// touched -- see the reverted "eng.dict gap-filler" commit). A word that DOES
/// have an authoritative answer ("mileage", "SKT") is untouched by this and
/// keeps using "eng"'s own trained answer.
#[test]
fn eng_substitutes_eng_us_only_for_words_with_no_authoritative_answer() {
    assert_eq!(
        korean_transliteration::transliterate("eng", "mileage").unwrap(),
        "마일리지"
    );
    assert_eq!(
        korean_transliteration::transliterate("eng", "SKT").unwrap(),
        "에스케이티"
    );
    // "photosynthesis" and "onboarding" have no hsl_eng/muik/korean_go entry --
    // this just confirms the substitution actually runs (matches "eng-us"'s own
    // decode of the same word), not any particular Hangul spelling.
    for word in ["photosynthesis", "onboarding"] {
        assert_eq!(
            korean_transliteration::transliterate("eng", word).unwrap(),
            korean_transliteration::transliterate("eng-us", word).unwrap(),
            "{word} should be decoded via eng-us, not eng's own misaki-only guess"
        );
    }
}
