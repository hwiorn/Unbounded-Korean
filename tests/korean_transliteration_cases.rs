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
