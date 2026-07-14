use g2pk::G2p;
use korean_phonemizer::{
    IpaStyle, PhonemizerMode, PhonemizerOptions, epitran_korean_to_ipa, korean_to_ipa,
    phonemize_ko_with_options,
};

fn g2p() -> G2p {
    G2p::new().unwrap()
}

#[test]
fn readme_sentence_cases() {
    let g2p = g2p();
    let cases = [
        (
            "학교에 갔다 와서, 엄마가 해 주신 밥을 먹었다.",
            "학꾜에 갇따 와서, 엄마가 해 주신 바블 머걷따.",
        ),
        (
            "나의 친구가 mp3 file 3개를 다운받고 있다",
            "나의 친구가 엠피쓰리 파일 세개를 다운받꼬 읻따",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(g2p.convert(input).unwrap(), expected, "{input}");
    }
}

#[test]
fn final_consonant_and_tensification_cases() {
    let g2p = g2p();
    let cases = [
        ("국밥", "국빱"),
        ("깎다", "깍따"),
        ("닦다", "닥따"),
        ("옷", "옫"),
        ("있다", "읻따"),
        ("꽃", "꼳"),
        ("앞", "압"),
        ("덮다", "덥따"),
        ("닭", "닥"),
        ("삯돈", "삭똔"),
        ("값지다", "갑찌다"),
    ];
    for (input, expected) in cases {
        assert_eq!(g2p.convert(input).unwrap(), expected, "{input}");
    }
}

#[test]
fn liaison_cases() {
    let g2p = g2p();
    let cases = [
        ("깎아", "까까"),
        ("옷이", "오시"),
        ("있어", "이써"),
        ("낮이", "나지"),
        ("꽂아", "꼬자"),
        ("꽃을", "꼬츨"),
        ("밭에", "바테"),
        ("앞으로", "아프로"),
        ("덮이다", "더피다"),
    ];
    for (input, expected) in cases {
        assert_eq!(g2p.convert(input).unwrap(), expected, "{input}");
    }
}

#[test]
fn phonemizer_python_basic_cases() {
    let opts = PhonemizerOptions {
        epitran_compat: false,
        ..PhonemizerOptions::default()
    };
    let cases = [
        ("가", "ka"),
        ("나", "na"),
        ("다", "ta"),
        ("아", "a"),
        ("자", "t͡ɕa"),
        ("차", "t͡ɕʰa"),
        ("카", "kʰa"),
        ("타", "tʰa"),
        ("파", "pʰa"),
        ("하", "ha"),
        ("말", "mal"),
    ];
    for (input, expected) in cases {
        assert_eq!(korean_to_ipa(input, &opts), expected, "{input}");
    }
}

#[test]
fn phonemizer_python_style_cases() {
    let combining = PhonemizerOptions {
        epitran_compat: false,
        ipa_style: IpaStyle::Combining,
        ..PhonemizerOptions::default()
    };
    let simplified = PhonemizerOptions {
        epitran_compat: false,
        ipa_style: IpaStyle::Simplified,
        ..PhonemizerOptions::default()
    };
    assert_eq!(korean_to_ipa("까", &combining), "k͈a");
    assert_eq!(korean_to_ipa("까", &simplified), "kka");
    assert_eq!(korean_to_ipa("짜", &combining), "t͈͡ɕa");
    assert_eq!(korean_to_ipa("짜", &simplified), "ttɕa");
    assert_eq!(
        PhonemizerMode::parse("epitran").unwrap(),
        PhonemizerMode::Epitran
    );
}

#[test]
fn phonemizer_epitran_mode_uses_rust_epitran() {
    assert_eq!(epitran_korean_to_ipa("한글").unwrap(), "hankɯl");
    let out = phonemize_ko_with_options(
        "한글",
        &PhonemizerOptions {
            mode: PhonemizerMode::Epitran,
            ..PhonemizerOptions::default()
        },
    )
    .unwrap();
    assert_eq!(out.ipa, "hankɯl");
}

#[test]
fn phonemizer_python_integration_cases() {
    let opts = PhonemizerOptions {
        epitran_compat: false,
        ..PhonemizerOptions::default()
    };
    let cases = [
        ("학교", "hak̚k͈jo"),
        ("국가", "kuk̚k͈a"),
        ("밥상", "pap̚s͈aŋ"),
        ("꽃다발", "k͈ot̚t͈abal"),
        ("같이", "kat͡ɕʰi"),
        ("굳이", "kud͡ɕi"),
        ("밭이", "pat͡ɕʰi"),
        ("말", "mal"),
        ("달", "tal"),
        ("설날", "sʌllal"),
        ("오기", "ogi"),
        ("아기", "agi"),
        ("바다", "pada"),
        ("가다", "kada"),
        ("아버지", "abʌd͡ɕi"),
        ("연락", "jʌllak̚"),
        ("신라", "silla"),
        ("논리", "nolli"),
        ("국민", "kuŋmin"),
        ("밥물", "pammul"),
        ("꽃이", "k͈ot͡ɕʰi"),
        ("밥을", "pabɯl"),
        ("옷을", "osɯl"),
        ("많아", "mana"),
        ("밖에", "pak͈e"),
    ];
    for (input, expected) in cases {
        let out = phonemize_ko_with_options(input, &opts).unwrap();
        assert_eq!(out.ipa, expected, "{input} -> {}", out.spoken);
    }
}
