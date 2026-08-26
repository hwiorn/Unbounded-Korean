mod hangul;
pub mod p2g;

use once_cell::sync::Lazy;
use phonetisaurus_g2p::PhonetisaurusModel;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no trained model for language: {0}")]
    ModelNotFound(String),
    #[error("g2p failed for {word:?}: {source}")]
    G2p { word: String, source: anyhow::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

static ENG_MODEL: Lazy<PhonetisaurusModel> = Lazy::new(|| {
    static BYTES: &[u8] = include_bytes!("../../../data/eng.fst");
    PhonetisaurusModel::try_from(BYTES).expect("bundled data/eng.fst must be a valid model")
});

fn model_for(lang: &str) -> Option<&'static PhonetisaurusModel> {
    match lang {
        "eng" | "en" => Some(&ENG_MODEL),
        _ => None,
    }
}

// The trained G2P model is a general letter-to-sound predictor: for an
// out-of-vocabulary all-caps token (an initialism/acronym) it guesses a
// word-shaped pronunciation instead of spelling the letters out, and for a
// proper noun with no real pronunciation rule to fall back on it guesses
// something unrelated to how the name is actually said. Both failure modes were
// already found and fixed the same way in hangulize-rs earlier this session; the
// same two-part fix is ported here rather than re-derived.
const ENGLISH_WORD_OVERRIDES: &[(&str, &str)] = &[("NAVER", "네이버")];

const ENGLISH_LETTER_NAMES: &[(char, &str)] = &[
    ('A', "에이"),
    ('B', "비"),
    ('C', "시"),
    ('D', "디"),
    ('E', "이"),
    ('F', "에프"),
    ('G', "지"),
    ('H', "에이치"),
    ('I', "아이"),
    ('J', "제이"),
    ('K', "케이"),
    ('L', "엘"),
    ('M', "엠"),
    ('N', "엔"),
    ('O', "오"),
    ('P', "피"),
    ('Q', "큐"),
    ('R', "아르"),
    ('S', "에스"),
    ('T', "티"),
    ('U', "유"),
    ('V', "브이"),
    ('W', "더블유"),
    ('X', "엑스"),
    ('Y', "와이"),
    ('Z', "지"),
];

fn is_all_caps_initialism(word: &str) -> bool {
    word.chars().count() >= 2 && word.chars().all(|c| c.is_ascii_uppercase())
}

fn spell_out_acronym(word: &str) -> String {
    word.chars()
        .filter_map(|c| {
            ENGLISH_LETTER_NAMES
                .iter()
                .find(|(letter, _)| *letter == c)
                .map(|(_, name)| *name)
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Transliterates `word` (in the source language identified by `lang`, e.g. "eng")
/// into Hangul, via a Phonetisaurus-trained G2P model plus table-driven P2G.
pub fn transliterate(lang: &str, word: &str) -> Result<String> {
    if lang == "eng" || lang == "en" {
        if let Some((_, hangul)) = ENGLISH_WORD_OVERRIDES
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(word))
        {
            return Ok((*hangul).to_string());
        }
        if is_all_caps_initialism(word) {
            return Ok(spell_out_acronym(word));
        }
    }

    let model = model_for(lang).ok_or_else(|| Error::ModelNotFound(lang.to_string()))?;
    let decoded = model
        .phonemize_word(word)
        .map_err(|source| Error::G2p {
            word: word.to_string(),
            source,
        })?;
    Ok(p2g::phonemes_to_hangul(&decoded.phonemes))
}
