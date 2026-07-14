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
        PhonemizerMode::Kog2pTable => korean_to_ipa(&spoken, options),
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

pub fn korean_to_ipa(spoken: &str, options: &PhonemizerOptions) -> String {
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

        // Lateralization (유음화): ㄴ+ㄹ and ㄹ+ㄴ surface as ㄹ+ㄹ.
        let lateral = matches!(
            (prev_coda, lead),
            (Some('ᆫ'), 'ᄅ') | (Some('ᆯ'), 'ᄂ') | (Some('ᆯ'), 'ᄅ')
        );

        // Progressive nasalization: ㅁ/ㅇ + ㄹ surfaces as ㅁ/ㅇ + ㄴ.
        let progressive_nasal = matches!((prev_coda, lead), (Some('ᆷ'), 'ᄅ') | (Some('ᆼ'), 'ᄅ'));

        // Onset ㄹ is [n] after progressive nasalization, [l] in lateral or
        // Epitran-compatible contexts, and [ɾ] medially otherwise.
        if lead == 'ᄅ' {
            if progressive_nasal {
                onset = "n".to_string();
            } else if lateral || options.epitran_compat || is_word_final(&chars, i + 1) {
                onset = "l".to_string();
            }
        } else if lead == 'ᄂ' && lateral {
            onset = "l".to_string();
        }

        let mut nucleus = medial_to_ipa(vowel).to_string();

        // ㅢ follows standard Korean pronunciation: word-initial [ɰi],
        // medial [i], with optional colloquial [ɛ] for possessive-like use.
        if vowel == 'ᅴ' {
            if at_word_start {
                nucleus = "ɰi".to_string();
            } else if options.colloquial && syllable_index > 0 {
                nucleus = "ɛ".to_string();
            } else {
                nucleus = "i".to_string();
            }
        }

        let mut coda = final_to_ipa(tail).to_string();
        // Epitran compatibility drops unreleased stop markers from codas.
        if options.epitran_compat {
            coda = coda.replace('\u{031a}', "");
        }

        // Palatalization: ㄷ/ㅌ before /i/ or yotized vowels becomes an
        // alveolo-palatal affricate outside word-initial position.
        if !at_word_start && is_palatal_vowel(vowel) {
            if lead == 'ᄃ' {
                onset = match options.ipa_style {
                    IpaStyle::Combining => "t͡ɕ".to_string(),
                    IpaStyle::Simplified => "tɕ".to_string(),
                };
            } else if lead == 'ᄐ' {
                onset = match options.ipa_style {
                    IpaStyle::Combining => "t͡ɕʰ".to_string(),
                    IpaStyle::Simplified => "tɕʰ".to_string(),
                };
            }
        }

        // Intervocalic/sonorant voicing: k/t/p and compatible affricates
        // become g/d/b/d͡ɕ after vowels or sonorant codas.
        if !at_word_start && is_voicing_context(prev_coda) {
            onset = voice_onset(&onset, options.ipa_style);
        }

        // Tensification (경음화): after selected codas, plain obstruent onsets
        // become tense if they have not already been changed by another rule.
        if !at_word_start && is_tensification_trigger(prev_coda) {
            onset = tensify(lead, &onset, options.ipa_style);
        }

        // When ㄴ+ㄹ lateralizes, the previous syllable's coda changes too.
        if lateral && prev_coda == Some('ᆫ') {
            if let Some(last) = result.last_mut() {
                *last = last.replace('n', "l");
            }
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

fn is_tensification_trigger(prev: Option<char>) -> bool {
    matches!(
        prev,
        Some('ᆨ' | 'ᆩ' | 'ᆪ' | 'ᆫ' | 'ᆮ' | 'ᆯ' | 'ᆷ' | 'ᆸ' | 'ᆹ' | 'ᆺ' | 'ᆻ' | 'ᆼ')
    )
}

fn tensify(lead: char, onset: &str, style: IpaStyle) -> String {
    let tense = match (lead, style) {
        ('ᄀ', IpaStyle::Combining) if onset == "k" => "k͈",
        ('ᄀ', IpaStyle::Simplified) if onset == "k" => "kk",
        ('ᄃ', IpaStyle::Combining) if onset == "t" => "t͈",
        ('ᄃ', IpaStyle::Simplified) if onset == "t" => "tt",
        ('ᄇ', IpaStyle::Combining) if onset == "p" => "p͈",
        ('ᄇ', IpaStyle::Simplified) if onset == "p" => "pp",
        ('ᄉ', IpaStyle::Combining) if onset == "s" => "s͈",
        ('ᄉ', IpaStyle::Simplified) if onset == "s" => "ss",
        ('ᄌ', IpaStyle::Combining) if onset == "t͡ɕ" => "t͈͡ɕ",
        ('ᄌ', IpaStyle::Simplified) if onset == "tɕ" => "ttɕ",
        _ => onset,
    };
    tense.to_string()
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
}
