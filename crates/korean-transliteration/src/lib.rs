mod hangul;
pub mod p2g;

use once_cell::sync::Lazy;
use sosap::Model;
use std::io::Read;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no trained model for language: {0}")]
    ModelNotFound(String),
    #[error("g2p produced no path for {word:?}")]
    NoPath { word: String },
}

pub type Result<T> = std::result::Result<T, Error>;

static ENG_MODEL: Lazy<Model> = Lazy::new(|| {
    static COMPRESSED: &[u8] = include_bytes!("../../../data/eng.fst.gz");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(COMPRESSED)
        .read_to_end(&mut bytes)
        .expect("bundled data/eng.fst.gz must be valid gzip");
    Model::from_bytes(&bytes, "").expect("decompressed data/eng.fst.gz must be a valid model")
});

fn model_for(lang: &str) -> Option<&'static Model> {
    match lang {
        "eng" | "en" => Some(&ENG_MODEL),
        _ => None,
    }
}

/// Transliterates `word` (in the source language identified by `lang`, e.g. "eng")
/// into Hangul, via a Phonetisaurus-trained G2P model plus table-driven P2G. No
/// per-word or per-pattern special-casing lives here: acronym/initialism spelling
/// and brand-name pronunciation are learned from training data (see
/// docs/plans/2026-08-26-korean-transliteration-plan.md), not hardcoded, so this
/// stays a plain conversion library.
pub fn transliterate(lang: &str, word: &str) -> Result<String> {
    let model = model_for(lang).ok_or_else(|| Error::ModelNotFound(lang.to_string()))?;
    let phonemes = model.phoneticize_simple(word);
    if phonemes.is_empty() {
        return Err(Error::NoPath {
            word: word.to_string(),
        });
    }
    Ok(p2g::phonemes_to_hangul(&phonemes))
}
