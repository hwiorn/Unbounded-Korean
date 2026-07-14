use unbounded_korean::{G2p, hangulize_rs, korean_phonemizer};

#[test]
fn rust_umbrella_reexports_core_crates() {
    let g2p = G2p::new().unwrap();
    assert_eq!(g2p.convert("국밥").unwrap(), "국빱");
    assert_eq!(hangulize_rs::hangulize("eng", "hello").unwrap(), "헬로");
    assert_eq!(
        korean_phonemizer::phonemize_ko("한글").unwrap().spoken,
        "한글"
    );
}
