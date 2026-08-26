use hangulize_rs::english_ipa_for_corpus;

#[test]
fn emits_simplified_ipa_for_common_words() {
    assert_eq!(english_ipa_for_corpus("hello").unwrap(), "h ə l oʊ");
    assert_eq!(english_ipa_for_corpus("text").unwrap(), "t ɛ k s t");
}

#[test]
fn strips_stress_marks_and_length_marks() {
    let ipa = english_ipa_for_corpus("world").unwrap();
    assert!(!ipa.contains('ˈ'));
    assert!(!ipa.contains('ː'));
    assert_eq!(ipa, "w ɜ l d");
}
