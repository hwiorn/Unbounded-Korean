use g2pk::{G2p, G2pConfig, G2pOptions, ResourceConfig};
use korean_phonemizer::{IpaStyle, PhonemizerOptions, korean_to_ipa};

#[test]
fn custom_idiom_resource_takes_precedence() {
    let g2p = G2p::with_config(G2pConfig {
        resources: ResourceConfig {
            table_csv: None,
            idioms_txt: Some("abc===가".to_string()),
        },
    })
    .unwrap();
    assert_eq!(g2p.convert("abc").unwrap(), "가");
}

#[test]
fn core_options_can_return_jamo() {
    let g2p = G2p::new().unwrap();
    let out = g2p
        .convert_with_options(
            "가",
            &G2pOptions {
                descriptive: false,
                group_vowels: false,
                to_syl: false,
            },
        )
        .unwrap();
    assert_eq!(out, "가");
}

#[test]
fn phonemizer_supports_combining_and_simplified_styles() {
    let combining = PhonemizerOptions::default();
    let simplified = PhonemizerOptions {
        ipa_style: IpaStyle::Simplified,
        ..PhonemizerOptions::default()
    };
    assert_eq!(korean_to_ipa("까", &combining), "k͈a");
    assert_eq!(korean_to_ipa("까", &simplified), "kka");
}
