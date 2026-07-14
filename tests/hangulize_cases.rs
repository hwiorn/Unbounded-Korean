use hangulize_rs::{Error, Hangulizer, hangulize, list_langs};
use std::fs;

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

#[test]
fn bundled_spec_test_matrix_matches() {
    for lang in list_langs() {
        let path = format!("crates/hangulize-rs/src/specs/{lang}.hsl");
        let src = fs::read_to_string(&path).unwrap();
        for (word, expected) in parse_hsl_tests(&src) {
            let actual =
                hangulize(&lang, &word).unwrap_or_else(|err| panic!("{lang}/{word}: {err}"));
            assert_eq!(actual, expected, "{lang}/{word}");
        }
    }
}

fn parse_hsl_tests(src: &str) -> Vec<(String, String)> {
    let mut in_test = false;
    let mut out = Vec::new();
    for raw in src.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(section) = line.strip_suffix(':') {
            in_test = section == "test";
            continue;
        }
        if !in_test {
            continue;
        }
        let Some((left, right)) = line.split_once("->") else {
            continue;
        };
        let word = parse_hsl_value(left);
        let expected = parse_hsl_value(right);
        out.push((word, expected));
    }
    out
}

fn strip_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or(line)
}

fn parse_hsl_value(src: &str) -> String {
    let src = src.trim();
    if !src.starts_with('"') {
        return src.to_string();
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in src[1..].chars() {
        if ch == '"' && !escaped {
            break;
        }
        if ch == '\\' && !escaped {
            escaped = true;
            continue;
        }
        escaped = false;
        out.push(ch);
    }
    out
}
