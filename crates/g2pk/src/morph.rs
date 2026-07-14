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
            let pos = details.first().copied().unwrap_or("UNK").to_string();
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
