use g2pk::hangul;
use g2pk::{Error as G2pError, G2p};
use once_cell::sync::OnceCell;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    G2p(#[from] G2pError),
    #[error("epitran transliteration failed: {0}")]
    Epitran(String),
    #[error("invalid phonemizer option: {0}")]
    InvalidOption(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpaStyle {
    /// Standard IPA representation using combining fortis and tie-bar marks.
    Combining,
    /// ASCII-friendlier tense consonants and affricates for limited vocabularies.
    Simplified,
}

impl IpaStyle {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "combining" => Ok(Self::Combining),
            "simplified" => Ok(Self::Simplified),
            other => Err(Error::InvalidOption(format!(
                "unsupported ipa_style: {other}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PhonemizerOptions {
    pub mode: PhonemizerMode,
    pub epitran_compat: bool,
    pub colloquial: bool,
    pub ipa_style: IpaStyle,
}

impl Default for PhonemizerOptions {
    fn default() -> Self {
        Self {
            mode: PhonemizerMode::Kog2pTable,
            epitran_compat: true,
            colloquial: false,
            ipa_style: IpaStyle::Combining,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhonemizerMode {
    Kog2pTable,
    Epitran,
}

impl PhonemizerMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "kog2p_table" => Ok(Self::Kog2pTable),
            "epitran" => Ok(Self::Epitran),
            other => Err(Error::InvalidOption(format!("unsupported mode: {other}"))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Phonemized {
    pub spoken: String,
    pub ipa: String,
}

pub fn phonemize_ko(text: &str) -> Result<Phonemized> {
    phonemize_ko_with_options(text, &PhonemizerOptions::default())
}

pub fn phonemize_ko_with_options(text: &str, options: &PhonemizerOptions) -> Result<Phonemized> {
    static G2P: OnceCell<G2p> = OnceCell::new();
    let g2p = G2P.get_or_try_init(G2p::new)?;
    let spoken = g2p.convert(text)?;
    let ipa = match options.mode {
        PhonemizerMode::Kog2pTable => {
            let josa_ui = josa_ui_for(g2p, text, &spoken);
            korean_to_ipa_with_josa(&spoken, options, &josa_ui)
        }
        PhonemizerMode::Epitran => epitran_korean_to_ipa(&spoken)?,
    };
    Ok(Phonemized { spoken, ipa })
}

pub fn epitran_korean_to_ipa(spoken: &str) -> Result<String> {
    static EPITRAN: OnceCell<rsepitran::Epitran> = OnceCell::new();
    let epitran = EPITRAN.get_or_init(rsepitran::Epitran::new);
    // The Korean Epitran table is keyed by compatibility jamo, so precomposed
    // Hangul syllables must be decomposed before FST transliteration.
    let compat = to_compat_jamo(spoken);
    epitran
        .transliterate_simple("kor_Hang", &compat)
        .map_err(|err| Error::Epitran(err.to_string()))
}

/// Syllable indexes of possessive 의 in `spoken`. The tagger runs on the original text, so the
/// indexes only carry over when both texts have the same number of Hangul syllables; otherwise
/// (numerals or English were rewritten) no 의 is treated as a particle.
fn josa_ui_for(g2p: &G2p, text: &str, spoken: &str) -> Vec<usize> {
    let syllables = |s: &str| s.chars().filter(|c| hangul::decompose_char(*c).is_some()).count();
    if syllables(text) != syllables(spoken) {
        return Vec::new();
    }
    g2p.josa_ui_syllables(text).unwrap_or_default()
}

pub fn korean_to_ipa(spoken: &str, options: &PhonemizerOptions) -> String {
    korean_to_ipa_with_josa(spoken, options, &[])
}

pub fn korean_to_ipa_with_josa(
    spoken: &str,
    options: &PhonemizerOptions,
    josa_ui: &[usize],
) -> String {
    let chars: Vec<char> = spoken.chars().collect();
    let mut result = Vec::new();
    let mut prev_coda: Option<char> = None;
    let mut at_word_start = true;
    let mut syllable_index = 0usize;

    for (i, ch) in chars.iter().enumerate() {
        if ch.is_whitespace() {
            result.push(ch.to_string());
            prev_coda = None;
            at_word_start = true;
            continue;
        }
        let Some((lead, vowel, tail)) = hangul::decompose_char(*ch) else {
            result.push(ch.to_string());
            continue;
        };

        let mut onset = initial_to_ipa(lead, options.ipa_style).to_string();

        // 유음화·ㄹ의 비음화·경음화는 g2pk spoken 형에 이미 반영돼 있다(단어 안에서만). 받침-초성
        // 인접으로 다시 적용하면 어절을 넘어 "를 넣"이 "를 러"로, 감사가 감싸로 바뀐다.
        // Onset ㄹ is [l] in Epitran-compatible or word-final contexts, [ɾ] medially otherwise.
        if lead == 'ᄅ' && (options.epitran_compat || is_word_final(&chars, i + 1)) {
            onset = "l".to_string();
        }

        let mut nucleus = medial_to_ipa(vowel).to_string();

        // ㅢ: word-initial [ɰi]; the possessive particle 의 is [ɰi] (colloquial [ɛ]); any other
        // medial ㅢ is [i].
        if vowel == 'ᅴ' {
            if at_word_start {
                nucleus = "ɰi".to_string();
            } else if josa_ui.contains(&syllable_index) {
                nucleus = if options.colloquial { "ɛ" } else { "ɰi" }.to_string();
            } else {
                nucleus = "i".to_string();
            }
        }

        let mut coda = final_to_ipa(tail).to_string();
        // Epitran compatibility drops unreleased stop markers from codas.
        if options.epitran_compat {
            coda = coda.replace('\u{031a}', "");
        }

        // Palatalization (구개음화): a syllable-FINAL ㄷ/ㅌ migrates into a following
        // null-onset 이/야/여/요/유 syllable as ㅈ/ㅊ -- 굳이[구지], 밭이[바치] (see
        // resources/rules.txt's citation of this rule). This is a coda absorbed by
        // the NEXT syllable, not a property of a syllable whose own onset happens to
        // be ㄷ/ㅌ (that onset never had a coda to migrate, so "어디"/"마디"/"느티나무"
        // must not palatalize).
        let palatalizes_coda = lead == 'ᄋ' && is_palatal_vowel(vowel);
        if palatalizes_coda && matches!(prev_coda, Some('ᆮ') | Some('ᇀ')) {
            onset = match (prev_coda, options.ipa_style) {
                (Some('ᆮ'), IpaStyle::Combining) => "d͡ɕ".to_string(),
                (Some('ᆮ'), IpaStyle::Simplified) => "dɕ".to_string(),
                (_, IpaStyle::Combining) => "t͡ɕʰ".to_string(),
                (_, IpaStyle::Simplified) => "tɕʰ".to_string(),
            };
            // The previous syllable's coda string, once absorbed here, must be
            // removed from what's already been pushed to `result` -- but that
            // string went through the same epitran_compat stripping `coda` above
            // does, so recompute it the same way rather than trusting
            // final_to_ipa(prev_coda)'s raw (un-stripped) length.
            let mut prev_coda_str = final_to_ipa(prev_coda).to_string();
            if options.epitran_compat {
                prev_coda_str = prev_coda_str.replace('\u{031a}', "");
            }
            if let Some(last) = result.last_mut() {
                last.truncate(last.len() - prev_coda_str.len());
            }
            prev_coda = None;
        }

        // Intervocalic/sonorant voicing: k/t/p and compatible affricates
        // become g/d/b/d͡ɕ after vowels or sonorant codas.
        if !at_word_start && is_voicing_context(prev_coda) {
            onset = voice_onset(&onset, options.ipa_style);
        }

        let syllable = format!("{onset}{nucleus}{coda}");
        if !syllable.is_empty() {
            result.push(syllable);
        }
        prev_coda = tail;
        at_word_start = false;
        syllable_index += 1;
    }

    result.concat().trim().to_string()
}

fn initial_to_ipa(ch: char, style: IpaStyle) -> &'static str {
    match (ch, style) {
        ('ᄀ', _) => "k",
        ('ᄁ', IpaStyle::Combining) => "k͈",
        ('ᄁ', IpaStyle::Simplified) => "kk",
        ('ᄂ', _) => "n",
        ('ᄃ', _) => "t",
        ('ᄄ', IpaStyle::Combining) => "t͈",
        ('ᄄ', IpaStyle::Simplified) => "tt",
        ('ᄅ', _) => "ɾ",
        ('ᄆ', _) => "m",
        ('ᄇ', _) => "p",
        ('ᄈ', IpaStyle::Combining) => "p͈",
        ('ᄈ', IpaStyle::Simplified) => "pp",
        ('ᄉ', _) => "s",
        ('ᄊ', IpaStyle::Combining) => "s͈",
        ('ᄊ', IpaStyle::Simplified) => "ss",
        ('ᄋ', _) => "",
        ('ᄌ', IpaStyle::Combining) => "t͡ɕ",
        ('ᄌ', IpaStyle::Simplified) => "tɕ",
        ('ᄍ', IpaStyle::Combining) => "t͈͡ɕ",
        ('ᄍ', IpaStyle::Simplified) => "ttɕ",
        ('ᄎ', IpaStyle::Combining) => "t͡ɕʰ",
        ('ᄎ', IpaStyle::Simplified) => "tɕʰ",
        ('ᄏ', _) => "kʰ",
        ('ᄐ', _) => "tʰ",
        ('ᄑ', _) => "pʰ",
        ('ᄒ', _) => "h",
        _ => "",
    }
}

fn medial_to_ipa(ch: char) -> &'static str {
    match ch {
        'ᅡ' => "a",
        'ᅢ' => "ɛ",
        'ᅣ' => "ja",
        'ᅤ' => "jɛ",
        'ᅥ' => "ʌ",
        'ᅦ' => "e",
        'ᅧ' => "jʌ",
        'ᅨ' => "je",
        'ᅩ' => "o",
        'ᅪ' => "wa",
        'ᅫ' => "wɛ",
        'ᅬ' => "we",
        'ᅭ' => "jo",
        'ᅮ' => "u",
        'ᅯ' => "wʌ",
        'ᅰ' => "we",
        'ᅱ' => "wi",
        'ᅲ' => "ju",
        'ᅳ' => "ɯ",
        'ᅴ' => "ɰi",
        'ᅵ' => "i",
        _ => "",
    }
}

fn final_to_ipa(ch: Option<char>) -> &'static str {
    match ch {
        None => "",
        Some('ᆨ' | 'ᆩ' | 'ᆪ' | 'ᆰ' | 'ᆿ') => "k̚",
        Some('ᆫ' | 'ᆬ' | 'ᆭ') => "n",
        Some('ᆮ' | 'ᆳ' | 'ᆴ' | 'ᆺ' | 'ᆻ' | 'ᆽ' | 'ᆾ' | 'ᇀ' | 'ᇂ') => "t̚",
        Some('ᆯ' | 'ᆶ') => "l",
        Some('ᆷ' | 'ᆱ') => "m",
        Some('ᆸ' | 'ᆲ' | 'ᆵ' | 'ᆹ' | 'ᇁ') => "p̚",
        Some('ᆼ') => "ŋ",
        _ => "",
    }
}

fn is_word_final(chars: &[char], start: usize) -> bool {
    for ch in chars.iter().skip(start) {
        if ch.is_whitespace() {
            return true;
        }
        if hangul::is_syllable(*ch) || hangul::is_lead(*ch) || hangul::is_vowel(*ch) {
            return false;
        }
    }
    true
}

fn is_palatal_vowel(ch: char) -> bool {
    matches!(ch, 'ᅵ' | 'ᅣ' | 'ᅧ' | 'ᅭ' | 'ᅲ' | 'ᅤ' | 'ᅨ')
}

fn is_voicing_context(prev: Option<char>) -> bool {
    prev.is_none() || matches!(prev, Some('ᆫ' | 'ᆯ' | 'ᆷ' | 'ᆼ'))
}

fn voice_onset(onset: &str, style: IpaStyle) -> String {
    match (onset, style) {
        ("k", _) => "g".to_string(),
        ("t", _) => "d".to_string(),
        ("p", _) => "b".to_string(),
        ("t͡ɕ", IpaStyle::Combining) => "d͡ɕ".to_string(),
        ("tɕ", IpaStyle::Simplified) => "dɕ".to_string(),
        _ => onset.to_string(),
    }
}

fn to_compat_jamo(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if let Some((lead, vowel, tail)) = hangul::decompose_char(ch) {
            out.push(compat_lead(lead));
            out.push(compat_vowel(vowel));
            if let Some(tail) = tail {
                out.push_str(compat_tail(tail));
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn compat_lead(ch: char) -> char {
    match ch {
        'ᄀ' => 'ㄱ',
        'ᄁ' => 'ㄲ',
        'ᄂ' => 'ㄴ',
        'ᄃ' => 'ㄷ',
        'ᄄ' => 'ㄸ',
        'ᄅ' => 'ㄹ',
        'ᄆ' => 'ㅁ',
        'ᄇ' => 'ㅂ',
        'ᄈ' => 'ㅃ',
        'ᄉ' => 'ㅅ',
        'ᄊ' => 'ㅆ',
        'ᄋ' => 'ㅇ',
        'ᄌ' => 'ㅈ',
        'ᄍ' => 'ㅉ',
        'ᄎ' => 'ㅊ',
        'ᄏ' => 'ㅋ',
        'ᄐ' => 'ㅌ',
        'ᄑ' => 'ㅍ',
        'ᄒ' => 'ㅎ',
        _ => ch,
    }
}

fn compat_vowel(ch: char) -> char {
    match ch {
        'ᅡ' => 'ㅏ',
        'ᅢ' => 'ㅐ',
        'ᅣ' => 'ㅑ',
        'ᅤ' => 'ㅒ',
        'ᅥ' => 'ㅓ',
        'ᅦ' => 'ㅔ',
        'ᅧ' => 'ㅕ',
        'ᅨ' => 'ㅖ',
        'ᅩ' => 'ㅗ',
        'ᅪ' => 'ㅘ',
        'ᅫ' => 'ㅙ',
        'ᅬ' => 'ㅚ',
        'ᅭ' => 'ㅛ',
        'ᅮ' => 'ㅜ',
        'ᅯ' => 'ㅝ',
        'ᅰ' => 'ㅞ',
        'ᅱ' => 'ㅟ',
        'ᅲ' => 'ㅠ',
        'ᅳ' => 'ㅡ',
        'ᅴ' => 'ㅢ',
        'ᅵ' => 'ㅣ',
        _ => ch,
    }
}

fn compat_tail(ch: char) -> &'static str {
    match ch {
        'ᆨ' => "ㄱ",
        'ᆩ' => "ㄲ",
        'ᆪ' => "ㄳ",
        'ᆫ' => "ㄴ",
        'ᆬ' => "ㄵ",
        'ᆭ' => "ㄶ",
        'ᆮ' => "ㄷ",
        'ᆯ' => "ㄹ",
        'ᆰ' => "ㄺ",
        'ᆱ' => "ㄻ",
        'ᆲ' => "ㄼ",
        'ᆳ' => "ㄽ",
        'ᆴ' => "ㄾ",
        'ᆵ' => "ㄿ",
        'ᆶ' => "ㅀ",
        'ᆷ' => "ㅁ",
        'ᆸ' => "ㅂ",
        'ᆹ' => "ㅄ",
        'ᆺ' => "ㅅ",
        'ᆻ' => "ㅆ",
        'ᆼ' => "ㅇ",
        'ᆽ' => "ㅈ",
        'ᆾ' => "ㅊ",
        'ᆿ' => "ㅋ",
        'ᇀ' => "ㅌ",
        'ᇁ' => "ㅍ",
        'ᇂ' => "ㅎ",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_spoken_korean_to_ipa() {
        let opts = PhonemizerOptions::default();
        assert_eq!(korean_to_ipa("가", &opts), "ka");
        assert_eq!(korean_to_ipa("강", &opts), "kaŋ");
    }

    #[test]
    fn supports_simplified_style() {
        let opts = PhonemizerOptions {
            ipa_style: IpaStyle::Simplified,
            ..PhonemizerOptions::default()
        };
        assert_eq!(korean_to_ipa("까", &opts), "kka");
    }

    #[test]
    fn rejects_invalid_style() {
        assert!(IpaStyle::parse("plain").is_err());
        assert!(PhonemizerMode::parse("unknown").is_err());
    }

    #[test]
    fn supports_epitran_mode() {
        assert_eq!(epitran_korean_to_ipa("한글").unwrap(), "hankɯl");
    }

    #[test]
    fn palatalizes_a_coda_d_or_t_absorbed_by_a_following_i_syllable() {
        // 구개음화 (palatalization): a syllable-FINAL ㄷ/ㅌ migrates into a following
        // null-onset 이/야/여/요/유 syllable as ㅈ/ㅊ -- 굳이[구지], 밭이[바치] (see
        // crates/g2pk/src/resources/rules.txt's own citation of this rule). This
        // requires an actual coda to consume; it is NOT a property of a syllable
        // whose own onset merely happens to be ㄷ/ㅌ.
        let opts = PhonemizerOptions::default();
        assert_eq!(korean_to_ipa("굳이", &opts), "kud͡ɕi");
        assert_eq!(korean_to_ipa("밭이", &opts), "pat͡ɕʰi");
    }

    #[test]
    fn does_not_palatalize_an_ordinary_d_or_t_onset_syllable() {
        // "어디" (where), "마디" (a joint) and "느티나무" (zelkova) each have ㄷ/ㅌ as a
        // syllable's OWN onset with no preceding coda to migrate -- there is nothing
        // to palatalize, and none of these are pronounced with ㅈ/ㅊ.
        let opts = PhonemizerOptions::default();
        assert_eq!(korean_to_ipa("어디", &opts), "ʌdi");
        assert_eq!(korean_to_ipa("마디", &opts), "madi");
        assert_eq!(korean_to_ipa("느티나무", &opts), "nɯtʰinamu");
    }

    #[test]
    fn does_not_reapply_assimilation_that_g2pk_already_decided() {
        let opts = PhonemizerOptions::default();
        assert!(korean_to_ipa("를 너", &opts).ends_with("nʌ"));
        assert!(korean_to_ipa("감 나", &opts).ends_with("na"));
        for word in ["감사", "인사", "선생", "방송", "관심"] {
            assert!(!korean_to_ipa(word, &opts).contains('͈'), "{word}");
        }
        assert!(korean_to_ipa("학꾜", &opts).contains('͈'));
    }

    #[test]
    fn ui_is_ɰi_for_particle_and_word_initial() {
        let josa = phonemize_ko("정부의 잘못이 아니다").unwrap().ipa;
        assert!(josa.starts_with("t͡ɕʌŋbuɰi "), "{josa}");
        let tagged_as_josa = phonemize_ko("건의사항 있으십니까").unwrap().ipa;
        assert!(tagged_as_josa.starts_with("kʌnɰisahaŋ "), "{tagged_as_josa}");
        let initial = phonemize_ko("의사").unwrap().ipa;
        assert!(initial.starts_with("ɰisa"), "{initial}");
    }

    #[test]
    fn adnominal_rieul_tenses_following_consonant() {
        // ko-dic fuses 하/VV + ᆯ/ETM into one token tagged "VV+ETM"; the ETM must still tense.
        let out = phonemize_ko("할 수 있습니다").unwrap().ipa;
        assert!(out.starts_with("hal s͈u"), "{out}");
        let out = phonemize_ko("하길 바라는").unwrap().ipa;
        assert!(out.starts_with("hagil pa"), "{out}");
    }
}
