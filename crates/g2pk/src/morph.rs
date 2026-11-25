use crate::{Error, Result};
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Morpheme {
    pub surface: String,
    pub pos: String,
    pub start: usize,
    pub end: usize,
}

pub trait PosTagger: Send + Sync {
    fn tag(&self, text: &str) -> Result<Vec<Morpheme>>;
}

pub struct LinderaTagger {
    tokenizer: Tokenizer,
}

impl LinderaTagger {
    pub fn new() -> Result<Self> {
        let dictionary = load_dictionary("embedded://ko-dic")
            .map_err(|err| Error::Morphology(err.to_string()))?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        Ok(Self {
            tokenizer: Tokenizer::new(segmenter),
        })
    }
}

impl PosTagger for LinderaTagger {
    fn tag(&self, text: &str) -> Result<Vec<Morpheme>> {
        let mut tokens = self
            .tokenizer
            .tokenize(text)
            .map_err(|err| Error::Morphology(err.to_string()))?;
        let mut out = Vec::with_capacity(tokens.len());
        for token in tokens.iter_mut() {
            let details = token.details();
            let pos = tag_of(&details);
            out.push(Morpheme {
                surface: token.surface.as_ref().to_string(),
                pos,
                start: token.byte_start,
                end: token.byte_end,
            });
        }
        Ok(out)
    }
}

/// ko-dic fuses an inflected stem and its trailing ending into one token ("할" = 하/VV + ᆯ/ETM).
/// When the ending is a lone final consonant it lives inside the last syllable, so that
/// syllable carries the ending's tag; otherwise the token keeps its leading component's tag.
fn tag_of(details: &[&str]) -> String {
    let leading = |pos: &str| pos.split('+').next().unwrap_or("UNK").to_string();
    let first = details.first().copied().unwrap_or("UNK");
    if let Some(expression) = details.get(7) {
        if let Some(last) = expression.split('+').last() {
            let mut parts = last.split('/');
            let surface = parts.next().unwrap_or("");
            let pos = parts.next().unwrap_or("");
            let mut chars = surface.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if ('\u{11A8}'..='\u{11C2}').contains(&c) && !pos.is_empty() {
                    return pos.to_string();
                }
            }
        }
    }
    leading(first)
}
