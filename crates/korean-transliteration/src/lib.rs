mod hangul;
pub mod p2g;

use once_cell::sync::Lazy;
use pinyin::ToPinyin;
use sosap::Model;
use std::collections::HashMap;
use std::io::Read;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no trained model for language: {0}")]
    ModelNotFound(String),
    #[error("g2p produced no path for {word:?}")]
    NoPath { word: String },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Decompresses and loads a bundled `.fst.gz` model. Every per-language model is
/// trained the same way (see scripts/train_phonetisaurus.sh): the training corpus is
/// always (source word, Korean-phoneme-space tokens) pairs, regardless of source
/// language, since the phonemes come from running each word's known-correct Hangul
/// answer through korean_phonemizer (see examples/hangul_answer_to_ipa_corpus.rs) --
/// so loading and decoding a model works identically for every language.
macro_rules! lang_model {
    ($name:ident, $path:literal) => {
        static $name: Lazy<Model> = Lazy::new(|| {
            static COMPRESSED: &[u8] = include_bytes!($path);
            let mut bytes = Vec::new();
            flate2::read::GzDecoder::new(COMPRESSED)
                .read_to_end(&mut bytes)
                .expect(concat!($path, " must be valid gzip"));
            Model::from_bytes(&bytes, "").expect(concat!($path, " must decompress to a valid model"))
        });
    };
}

lang_model!(ENG_MODEL, "../../../data/eng.fst.gz");
lang_model!(NLD_MODEL, "../../../data/nld.fst.gz");
lang_model!(ITA_MODEL, "../../../data/ita.fst.gz");
lang_model!(DEU_MODEL, "../../../data/deu.fst.gz");
lang_model!(SPA_MODEL, "../../../data/spa.fst.gz");
lang_model!(CHI_MODEL, "../../../data/chi.fst.gz");
lang_model!(JPN_MODEL, "../../../data/jpn.fst.gz");
lang_model!(AZE_MODEL, "../../../data/aze.fst.gz");
lang_model!(BEL_MODEL, "../../../data/bel.fst.gz");
lang_model!(BUL_MODEL, "../../../data/bul.fst.gz");
lang_model!(CAT_MODEL, "../../../data/cat.fst.gz");
lang_model!(CES_MODEL, "../../../data/ces.fst.gz");
lang_model!(CYM_MODEL, "../../../data/cym.fst.gz");
lang_model!(ELL_MODEL, "../../../data/ell.fst.gz");
lang_model!(EPO_MODEL, "../../../data/epo.fst.gz");
lang_model!(EST_MODEL, "../../../data/est.fst.gz");
lang_model!(FIN_MODEL, "../../../data/fin.fst.gz");
lang_model!(GRC_MODEL, "../../../data/grc.fst.gz");
lang_model!(HBS_MODEL, "../../../data/hbs.fst.gz");
lang_model!(HUN_MODEL, "../../../data/hun.fst.gz");
lang_model!(ISL_MODEL, "../../../data/isl.fst.gz");
lang_model!(KAT_MODEL, "../../../data/kat.fst.gz");
lang_model!(LAT_MODEL, "../../../data/lat.fst.gz");
lang_model!(LAV_MODEL, "../../../data/lav.fst.gz");
lang_model!(LIT_MODEL, "../../../data/lit.fst.gz");
lang_model!(MKD_MODEL, "../../../data/mkd.fst.gz");
lang_model!(POL_MODEL, "../../../data/pol.fst.gz");
lang_model!(POR_MODEL, "../../../data/por.fst.gz");
lang_model!(RON_MODEL, "../../../data/ron.fst.gz");
lang_model!(RUS_MODEL, "../../../data/rus.fst.gz");
lang_model!(SLK_MODEL, "../../../data/slk.fst.gz");
lang_model!(SLV_MODEL, "../../../data/slv.fst.gz");
lang_model!(SQI_MODEL, "../../../data/sqi.fst.gz");
lang_model!(SWE_MODEL, "../../../data/swe.fst.gz");
lang_model!(TUR_MODEL, "../../../data/tur.fst.gz");
lang_model!(UKR_MODEL, "../../../data/ukr.fst.gz");
lang_model!(VIE_MODEL, "../../../data/vie.fst.gz");
lang_model!(WLM_MODEL, "../../../data/wlm.fst.gz");

fn model_for(lang: &str) -> Option<&'static Model> {
    match lang {
        "eng" | "en" => Some(&ENG_MODEL),
        "nld" | "nl" => Some(&NLD_MODEL),
        "ita" | "it" => Some(&ITA_MODEL),
        "deu" | "de" => Some(&DEU_MODEL),
        "spa" | "es" => Some(&SPA_MODEL),
        "chi" | "zh" => Some(&CHI_MODEL),
        "jpn" | "ja" => Some(&JPN_MODEL),
        "aze" | "az" => Some(&AZE_MODEL),
        "bel" | "be" => Some(&BEL_MODEL),
        "bul" | "bg" => Some(&BUL_MODEL),
        "cat" | "ca" => Some(&CAT_MODEL),
        "ces" | "cs" => Some(&CES_MODEL),
        "cym" | "cy" => Some(&CYM_MODEL),
        "ell" | "el" => Some(&ELL_MODEL),
        "epo" | "eo" => Some(&EPO_MODEL),
        "est" | "et" => Some(&EST_MODEL),
        "fin" | "fi" => Some(&FIN_MODEL),
        "grc" => Some(&GRC_MODEL),
        "hbs" => Some(&HBS_MODEL),
        "hun" | "hu" => Some(&HUN_MODEL),
        "isl" | "is" => Some(&ISL_MODEL),
        "kat" | "ka" => Some(&KAT_MODEL),
        "lat" | "la" => Some(&LAT_MODEL),
        "lav" | "lv" => Some(&LAV_MODEL),
        "lit" | "lt" => Some(&LIT_MODEL),
        "mkd" | "mk" => Some(&MKD_MODEL),
        "pol" | "pl" => Some(&POL_MODEL),
        "por" | "pt" => Some(&POR_MODEL),
        "ron" | "ro" => Some(&RON_MODEL),
        "rus" | "ru" => Some(&RUS_MODEL),
        "slk" | "sk" => Some(&SLK_MODEL),
        "slv" | "sl" => Some(&SLV_MODEL),
        "sqi" | "sq" => Some(&SQI_MODEL),
        "swe" | "sv" => Some(&SWE_MODEL),
        "tur" | "tr" => Some(&TUR_MODEL),
        "ukr" | "uk" => Some(&UKR_MODEL),
        "vie" | "vi" => Some(&VIE_MODEL),
        "wlm" => Some(&WLM_MODEL),
        _ => None,
    }
}

/// Parses a bundled `word<TAB>hangul` resource into an exact-match lookup table.
fn parse_dictionary(tsv: &'static str) -> HashMap<&'static str, &'static str> {
    tsv.lines()
        .filter_map(|line| line.split_once('\t'))
        .collect()
}

/// Chinese and Japanese are logographic: a Hanzi/Kanji character doesn't decompose
/// into letter-sized sound units, so Phonetisaurus's grapheme-to-phoneme alignment
/// can't generalize from it directly the way it can for an alphabetic spelling (see
/// examples/romanize_chi_corpus.rs and examples/romanize_jpn_corpus.rs). Each of these
/// languages' models is trained on a romanized proxy spelling instead, so a known word
/// is looked up directly against the exact (word, Hangul) pairs the model itself was
/// trained from -- guaranteed correct, no model uncertainty -- and only a word outside
/// that list falls through to romanize-then-G2P, which is a genuine guess.
fn dictionary_for(lang: &str) -> Option<&'static HashMap<&'static str, &'static str>> {
    static CHI_DICT: Lazy<HashMap<&str, &str>> =
        Lazy::new(|| parse_dictionary(include_str!("../resources/chi_dictionary.tsv")));
    static JPN_DICT: Lazy<HashMap<&str, &str>> =
        Lazy::new(|| parse_dictionary(include_str!("../resources/jpn_dictionary.tsv")));
    match lang {
        "chi" | "zh" => Some(&CHI_DICT),
        "jpn" | "ja" => Some(&JPN_DICT),
        _ => None,
    }
}

/// Converts a logographic word into the alphabetic-adjacent proxy spelling its G2P
/// model was trained on (see `dictionary_for`'s doc comment) -- concatenated plain
/// pinyin for Chinese, or a kana reading (family/given name space stripped, matching
/// how the training data itself was built) for Japanese. Returns the word unchanged
/// for every other language, which is already a spelling the model can use directly.
fn romanize(lang: &str, word: &str) -> String {
    match lang {
        "chi" | "zh" => word
            .chars()
            .filter_map(|c| c.to_pinyin())
            .map(|p| p.plain())
            .collect(),
        "jpn" | "ja" => hangulize_rs::kana_reading_for_corpus(word)
            .map(|reading| reading.chars().filter(|c| !c.is_whitespace()).collect())
            .unwrap_or_default(),
        _ => word.to_string(),
    }
}

/// Transliterates `word` (in the source language identified by `lang`, e.g. "eng")
/// into Hangul, via a Phonetisaurus-trained G2P model plus table-driven P2G. No
/// per-word or per-pattern special-casing lives here: acronym/initialism spelling
/// and brand-name pronunciation are learned from training data (see
/// docs/plans/2026-08-26-korean-transliteration-plan.md), not hardcoded, so this
/// stays a plain conversion library -- except Chinese/Japanese's exact-match
/// dictionary fast path, which exists because their models can't generalize the way
/// an alphabetic language's can (see `dictionary_for`).
pub fn transliterate(lang: &str, word: &str) -> Result<String> {
    if let Some(hangul) = dictionary_for(lang).and_then(|dict| dict.get(word)) {
        return Ok((*hangul).to_string());
    }
    let model = model_for(lang).ok_or_else(|| Error::ModelNotFound(lang.to_string()))?;
    let romanized = romanize(lang, word);
    let phonemes = model.phoneticize_simple(&romanized);
    if phonemes.is_empty() {
        return Err(Error::NoPath {
            word: word.to_string(),
        });
    }
    Ok(p2g::phonemes_to_hangul(&phonemes))
}
