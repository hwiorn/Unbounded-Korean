use crate::Result;
use crate::english;
use crate::hangul;
use crate::morph::{LinderaTagger, PosTagger};
use crate::numerals;
use crate::resources::{ResourceConfig, Resources};
use crate::rules;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct G2pOptions {
    pub descriptive: bool,
    pub group_vowels: bool,
    pub to_syl: bool,
}

impl Default for G2pOptions {
    fn default() -> Self {
        Self {
            descriptive: false,
            group_vowels: false,
            to_syl: true,
        }
    }
}

impl G2pOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct G2pConfig {
    pub resources: ResourceConfig,
}

pub struct G2p {
    resources: Resources,
    tagger: Arc<dyn PosTagger>,
}

impl G2p {
    pub fn new() -> Result<Self> {
        Self::with_config(G2pConfig::default())
    }

    pub fn with_config(config: G2pConfig) -> Result<Self> {
        Ok(Self {
            resources: Resources::load(&config.resources)?,
            tagger: Arc::new(LinderaTagger::new()?),
        })
    }

    pub fn convert(&self, text: &str) -> Result<String> {
        self.convert_with_options(text, &G2pOptions::new())
    }

    pub fn convert_with_options(&self, text: &str, options: &G2pOptions) -> Result<String> {
        let mut out = self.apply_idioms(text)?;
        out = english::convert_eng(&out);
        out = self.annotate(&out)?;
        out = numerals::convert_num(&out);
        out = hangul::decompose(&out);
        out = rules::apply_special(out, options.descriptive);
        out = rules::strip_markers(&out);
        out = rules::apply_table(out, &self.resources.table);
        out = rules::apply_links(out);
        if options.group_vowels {
            out = rules::group_vowels(&out);
        }
        if options.to_syl {
            out = hangul::compose(&out);
        }
        Ok(out)
    }

    fn apply_idioms(&self, text: &str) -> Result<String> {
        let mut out = text.to_string();
        for (regex, to) in &self.resources.idioms {
            out = regex.replace_all(&out, to.as_str()).to_string();
        }
        Ok(out)
    }

    fn annotate(&self, text: &str) -> Result<String> {
        let morphemes = self.tagger.tag(text)?;
        let tags = morphemes
            .into_iter()
            .map(|m| (m.surface, m.pos))
            .collect::<Vec<_>>();
        Ok(rules::annotate_by_tags(text, &tags))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_with_custom_idiom() {
        let g2p = G2p::with_config(G2pConfig {
            resources: ResourceConfig {
                table_csv: None,
                idioms_txt: Some("테스트===데스트".to_string()),
            },
        })
        .unwrap();
        assert!(g2p.convert("테스트").unwrap().contains('데'));
    }

    #[test]
    fn converts_numbers_and_common_english() {
        let g2p = G2p::new().unwrap();
        let out = g2p.convert("file 3개").unwrap();
        assert!(out.contains("파일"));
        assert!(out.contains("세개"));
    }
}
