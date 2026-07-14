use hangulize_rs::{Error, Hangulizer, hangulize, list_langs};

#[test]
fn lists_bundled_languages_sorted() {
    let langs = list_langs();
    assert!(langs.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(langs.contains(&"ita".to_string()));
    assert!(langs.contains(&"jpn".to_string()));
    assert!(langs.contains(&"rus".to_string()));
}

#[test]
fn hangulizes_representative_italian_examples() {
    assert_eq!(hangulize("ita", "Cappuccino").unwrap(), "카푸치노");
    assert_eq!(hangulize("ita", "glor/ia").unwrap(), "글로르/이아");
    assert_eq!(hangulize("ita", "glor,ia").unwrap(), "글로르,이아");
}

#[test]
fn hangulizes_latin_script_examples() {
    assert_eq!(
        hangulize("ron", "Cătălin Moroşanu").unwrap(),
        "커털린 모로샤누"
    );
    assert_eq!(
        hangulize("nld", "Jerrel Venetiaan").unwrap(),
        "예럴 페네티안"
    );
    assert_eq!(
        hangulize("por", "Vítor Constâncio").unwrap(),
        "비토르 콘스탄시우"
    );
}

#[test]
fn hangulizes_noop_translit_script_examples() {
    assert_eq!(hangulize("jpn", "ウィ").unwrap(), "위");
    assert_eq!(hangulize("jpn", "ヴァ").unwrap(), "바");
    assert_eq!(hangulize("chi", "quan").unwrap(), "취안");
}

#[test]
fn hangulizes_cyrillic_translit_examples() {
    assert_eq!(hangulize("rus", "Владивосток").unwrap(), "블라디보스토크");
    assert_eq!(hangulize("rus", "Путин").unwrap(), "푸틴");
    assert_eq!(hangulize("bul", "Пловдив").unwrap(), "플로브디프");
    assert_eq!(hangulize("mkd", "Кичево").unwrap(), "키체보");
    assert_eq!(hangulize("ukr", "Київ").unwrap(), "키이우");
}

#[test]
fn hangulizer_reuses_loaded_spec() {
    let h = Hangulizer::new("ita").unwrap();
    assert_eq!(h.lang(), "ita");
    assert_eq!(h.hangulize("Vivace").unwrap(), "비바체");
}

#[test]
fn reports_unknown_language() {
    let err = hangulize("unknown", "hello").unwrap_err();
    assert!(matches!(err, Error::SpecNotFound(lang) if lang == "unknown"));
}
