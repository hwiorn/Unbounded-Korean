mod specs_generated {
    include!(concat!(env!("OUT_DIR"), "/specs_generated.rs"));
}

use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use once_cell::sync::Lazy;
use pinyin::ToPinyin;
use regex::{Captures, Regex};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

const CYRILLIC_DICT: &str = include_str!("translit/cyrillic_dict.json");

#[derive(Debug, Error)]
pub enum Error {
    #[error("spec not found: {0}")]
    SpecNotFound(String),
    #[error("invalid spec: {0}")]
    InvalidSpec(String),
    #[error("invalid rule: {0}")]
    InvalidRule(String),
    #[error("translit not available: {0}")]
    TranslitNotAvailable(String),
}

pub type Result<T> = std::result::Result<T, Error>;

static SPEC_CACHE: Lazy<std::sync::Mutex<HashMap<String, Arc<Spec>>>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

pub fn list_langs() -> Vec<String> {
    let mut langs = specs_generated::SPECS
        .iter()
        .map(|(lang, _)| (*lang).to_string())
        .collect::<Vec<_>>();
    langs.sort();
    langs
}

pub fn load_spec(lang: &str) -> Result<Arc<Spec>> {
    if let Some(spec) = SPEC_CACHE.lock().unwrap().get(lang).cloned() {
        return Ok(spec);
    }
    let Some((_, src)) = specs_generated::SPECS
        .iter()
        .find(|(candidate, _)| *candidate == lang)
    else {
        return Err(Error::SpecNotFound(lang.to_string()));
    };
    let spec = Arc::new(Spec::parse(src)?);
    SPEC_CACHE
        .lock()
        .unwrap()
        .insert(lang.to_string(), Arc::clone(&spec));
    Ok(spec)
}

pub fn hangulize(lang: &str, word: &str) -> Result<String> {
    Hangulizer::new(lang)?.hangulize(word)
}

#[derive(Clone)]
pub struct Hangulizer {
    spec: Arc<Spec>,
}

impl Hangulizer {
    pub fn new(lang: &str) -> Result<Self> {
        Ok(Self {
            spec: load_spec(lang)?,
        })
    }

    pub fn lang(&self) -> &str {
        &self.spec.lang.id
    }

    pub fn hangulize(&self, word: &str) -> Result<String> {
        let word = self.transliterate(word)?;
        let word = self.normalize(&word);
        let subwords = self.partition(&word);
        let subwords = self.rewrite(subwords)?;
        let subwords = self.transcribe(subwords)?;
        let word = syllabify(&subwords);
        Ok(self.localize(&word))
    }

    fn transliterate(&self, word: &str) -> Result<String> {
        let mut out = word.to_string();
        for scheme in &self.spec.lang.translit {
            match scheme.as_str() {
                "pinyin" => {
                    out = transliterate_pinyin(&out);
                }
                "furigana" => {
                    out = transliterate_furigana(&out, self.spec.lang.id == "jpn")?;
                }
                "english_phoneme" => {
                    out = transliterate_english_phoneme(&out)?;
                }
                other if other.starts_with("cyrillic[") && other.ends_with(']') => {
                    let country = &other["cyrillic[".len()..other.len() - 1];
                    out = transliterate_cyrillic(country, &out)?;
                }
                other => return Err(Error::TranslitNotAvailable(other.to_string())),
            }
        }
        Ok(out)
    }

    fn normalize(&self, word: &str) -> String {
        let mut out = self.spec.norm_replace(word);
        out = out
            .chars()
            .map(|ch| {
                if self.spec.norm_letters.contains_key(&ch) || !self.spec.script.is(ch) {
                    ch
                } else {
                    self.spec.script.normalize(ch)
                }
            })
            .collect();
        out
    }

    fn partition(&self, word: &str) -> Vec<Subword> {
        let mut rep = Replacer::new(word, 0, 1);
        for (i, ch) in word.char_indices() {
            let end = i + ch.len_utf8();
            if self.spec.script.is(ch) || is_rule_separator(ch) || is_space_only(ch) {
                rep.replace(i, end, &ch.to_string());
            }
        }
        rep.subwords()
    }

    fn rewrite(&self, subwords: Vec<Subword>) -> Result<Vec<Subword>> {
        let mut builder = SubwordBuilder::default();
        for sw in subwords {
            let mut word = sw.word;
            let mut levels = vec![sw.level; word.len()];
            for rule in &self.spec.rewrite {
                let repls = rule.replacements(&word)?;
                let mut rep = Replacer::with_levels(&word, levels, 1);
                rep.replace_by(repls);
                let parts = rep.into_parts();
                word = parts.0;
                levels = parts.1;
            }
            let mut rep = Replacer::with_levels(&word, levels, 1);
            builder.write(rep.subwords());
        }
        Ok(builder.subwords())
    }

    fn transcribe(&self, subwords: Vec<Subword>) -> Result<Vec<Subword>> {
        let mut builder = SubwordBuilder::default();
        for sw in subwords {
            if sw.level == 0 {
                builder.write(vec![sw]);
                continue;
            }
            let mut word = sw.word.clone();
            let mut levels = vec![sw.level; word.len()];
            let mut dummy_word = sw.word;
            for rule in &self.spec.transcribe {
                let repls = rule.replacements(&dummy_word)?;
                let mut rep = Replacer::with_levels(&word, levels, 2);
                for repl in &repls {
                    rep.replace(repl.start, repl.stop, &repl.word);
                }
                let (next_word, next_levels) = rep.into_parts();
                word = next_word;
                levels = next_levels;

                let mut dummy = Replacer::new(&dummy_word, 0, 0);
                for repl in &repls {
                    dummy.replace(repl.start, repl.stop, &"\0".repeat(repl.word.len()));
                }
                dummy_word = dummy.string();
            }
            let mut rep = Replacer::with_levels(&word, levels, 2);
            builder.write(rep.subwords());
        }

        let mut cleaned = SubwordBuilder::default();
        for sw in builder.subwords() {
            if sw.level == 1 {
                if sw.word.chars().any(char::is_whitespace) {
                    cleaned.write(vec![Subword::new(" ", 1)]);
                }
            } else {
                cleaned.write(vec![sw]);
            }
        }
        Ok(cleaned.subwords())
    }

    fn localize(&self, word: &str) -> String {
        let chars = word.chars().collect::<Vec<_>>();
        let last = chars.len().saturating_sub(1);
        let mut out = String::new();
        for (i, ch) in chars.iter().copied().enumerate() {
            if ch == '\u{200b}' {
                continue;
            }
            if !is_punct(ch) {
                out.push(ch);
                continue;
            }
            let mut punct = self.spec.script.localize_punct(ch);
            let left_trim = i == 0 || is_punct(chars[i - 1]) || chars[i - 1].is_whitespace();
            let right_trim = i == last || is_punct(chars[i + 1]) || chars[i + 1].is_whitespace();
            if left_trim {
                punct = punct.trim_start().to_string();
            }
            if right_trim {
                punct = punct.trim_end().to_string();
            }
            out.push_str(&punct);
        }
        out
    }
}

#[derive(Clone)]
pub struct Spec {
    lang: Language,
    normalize: HashMap<String, Vec<String>>,
    rewrite: Vec<Rule>,
    transcribe: Vec<Rule>,
    script: Script,
    norm_letters: HashMap<char, bool>,
}

impl Spec {
    fn parse(src: &str) -> Result<Self> {
        let hsl = parse_hsl(src)?;
        let lang = Language::from_hsl(hsl.get("lang"))?;
        let macros = hsl
            .get("macros")
            .map(dict_one_to_one)
            .transpose()?
            .unwrap_or_default();
        let vars = hsl.get("vars").map(dict_map).unwrap_or_default();
        let normalize = hsl.get("normalize").map(dict_map).unwrap_or_default();
        let rewrite = rules_from_pairs(hsl.get("rewrite"), &macros, &vars)?;
        let transcribe = rules_from_pairs(hsl.get("transcribe"), &macros, &vars)?;
        let script = Script::from_name(&lang.script);
        let norm_letters = normalize
            .keys()
            .flat_map(|key| key.chars())
            .map(|ch| (ch, true))
            .collect();
        Ok(Self {
            lang,
            normalize,
            rewrite,
            transcribe,
            script,
            norm_letters,
        })
    }

    fn norm_replace(&self, word: &str) -> String {
        let mut out = word.to_string();
        for (to, froms) in &self.normalize {
            for from in froms {
                out = out.replace(from, to);
            }
        }
        out
    }
}

#[derive(Clone, Default)]
struct Language {
    id: String,
    script: String,
    translit: Vec<String>,
}

impl Language {
    fn from_hsl(section: Option<&Section>) -> Result<Self> {
        let Some(Section::Dict(pairs)) = section else {
            return Ok(Self::default());
        };
        Ok(Self {
            id: one(pairs, "id"),
            script: one(pairs, "script"),
            translit: all(pairs, "translit"),
        })
    }
}

#[derive(Clone)]
struct Rule {
    from: Pattern,
    to: RPattern,
}

impl Rule {
    fn replacements(&self, word: &str) -> Result<Vec<Replacement>> {
        let mut out = Vec::new();
        for found in self.from.find(word)? {
            let repl = self.to.interpolate(&self.from, word, &found)?;
            out.push(Replacement::new(found.start, found.stop, &repl));
        }
        Ok(out)
    }
}

fn rules_from_pairs(
    section: Option<&Section>,
    macros: &HashMap<String, String>,
    vars: &HashMap<String, Vec<String>>,
) -> Result<Vec<Rule>> {
    let Some(Section::List(pairs)) = section else {
        return Ok(Vec::new());
    };
    pairs
        .iter()
        .map(|pair| {
            Ok(Rule {
                from: Pattern::new(&pair.left, macros, vars)?,
                to: RPattern::new(
                    pair.right.first().map(String::as_str).unwrap_or(""),
                    macros,
                    vars,
                ),
            })
        })
        .collect()
}

#[derive(Clone)]
struct Pattern {
    expr: String,
    re: Regex,
    neg_a: Option<Regex>,
    neg_b: Option<Regex>,
    neg_a_width: isize,
    neg_b_width: isize,
    used_vars: Vec<Vec<String>>,
}

impl Pattern {
    fn new(
        expr: &str,
        macros: &HashMap<String, String>,
        vars: &HashMap<String, Vec<String>>,
    ) -> Result<Self> {
        if expr.is_empty() {
            return Err(Error::InvalidRule("empty pattern".to_string()));
        }
        let mut re_expr = expand_macros(expr, macros);
        let (expanded, used_vars) = expand_vars(&re_expr, vars);
        re_expr = expanded;
        let (pos, neg_a, neg_b, neg_a_width, neg_b_width) = expand_lookaround(&re_expr)?;
        let re_expr = escape_zero_width_space(&expand_edges(&pos));
        let re =
            Regex::new(&re_expr).map_err(|err| Error::InvalidRule(format!("{expr}: {err}")))?;
        let neg_a = compile_optional(neg_a.as_deref())?;
        let neg_b = compile_optional(neg_b.as_deref())?;
        Ok(Self {
            expr: expr.to_string(),
            re,
            neg_a,
            neg_b,
            neg_a_width,
            neg_b_width,
            used_vars,
        })
    }

    fn find(&self, word: &str) -> Result<Vec<Found>> {
        let mut matches = Vec::new();
        let mut offset = 0;
        while offset < word.len() {
            let Some(caps) = self.re.captures(&word[offset..]) else {
                break;
            };
            let positions = capture_positions(&caps, offset);
            if positions.is_empty() {
                break;
            }
            let group_count = positions.len();
            if group_count < 5 {
                return Err(Error::InvalidRule(format!(
                    "unexpected captures from {}",
                    self.expr
                )));
            }
            let full = positions[0].unwrap();
            let start = positions
                .get(2)
                .and_then(|p| *p)
                .map(|(_, end)| end)
                .unwrap_or(full.0);
            let stop = positions
                .get(group_count.saturating_sub(2))
                .and_then(|p| *p)
                .map(|(start, _)| start)
                .unwrap_or(full.1);
            if stop <= start {
                return Err(Error::InvalidRule(format!(
                    "zero-width match from {}",
                    self.expr
                )));
            }
            offset = stop;

            if let Some(neg_a) = &self.neg_a {
                let Some((neg_start, _)) = positions[group_count - 2] else {
                    continue;
                };
                let neg_stop = if self.neg_a_width == -1 {
                    word.len()
                } else {
                    (neg_start + self.neg_a_width as usize).min(word.len())
                };
                if neg_a.is_match(substr(word, neg_start, neg_stop)) {
                    continue;
                }
            }
            if let Some(neg_b) = &self.neg_b {
                let Some((_, neg_stop)) = positions[2] else {
                    continue;
                };
                let neg_start = if self.neg_b_width == -1 {
                    0
                } else {
                    neg_stop.saturating_sub(self.neg_b_width as usize)
                };
                if neg_b.is_match(substr(word, neg_start, neg_stop)) {
                    continue;
                }
            }

            let core = if group_count > 5 {
                positions[3..group_count - 2].to_vec()
            } else {
                Vec::new()
            };
            matches.push(Found { start, stop, core });
        }
        Ok(matches)
    }
}

#[derive(Clone)]
struct Found {
    start: usize,
    stop: usize,
    core: Vec<Option<(usize, usize)>>,
}

#[derive(Clone)]
struct RPattern {
    parts: Vec<RPart>,
}

#[derive(Clone)]
enum RPart {
    Plain(String),
    Var(Vec<String>),
}

impl RPattern {
    fn new(
        expr: &str,
        macros: &HashMap<String, String>,
        vars: &HashMap<String, Vec<String>>,
    ) -> Self {
        let expr = expand_macros(expr, macros);
        let re_var = Regex::new(r"<(.+?)>").unwrap();
        let mut parts = Vec::new();
        let mut offset = 0;
        for caps in re_var.captures_iter(&expr) {
            let whole = caps.get(0).unwrap();
            if whole.start() > offset {
                parts.push(RPart::Plain(expr[offset..whole.start()].to_string()));
            }
            let name = caps.get(1).unwrap().as_str();
            parts.push(RPart::Var(vars.get(name).cloned().unwrap_or_default()));
            offset = whole.end();
        }
        if offset < expr.len() {
            parts.push(RPart::Plain(expr[offset..].to_string()));
        }
        Self { parts }
    }

    fn interpolate(&self, pattern: &Pattern, word: &str, found: &Found) -> Result<String> {
        let mut out = String::new();
        let mut var_index = 0;
        for part in &self.parts {
            match part {
                RPart::Plain(text) => out.push_str(text),
                RPart::Var(to_var) => {
                    let from_var = pattern.used_vars.get(var_index).ok_or_else(|| {
                        Error::InvalidRule("mapped vars have different length".to_string())
                    })?;
                    let captured = found
                        .core
                        .get(var_index)
                        .and_then(|p| *p)
                        .map(|(start, stop)| substr(word, start, stop))
                        .unwrap_or("");
                    let idx = from_var.iter().position(|val| val == captured).unwrap_or(0);
                    if !to_var.is_empty() {
                        out.push_str(&to_var[idx % to_var.len()]);
                    }
                    var_index += 1;
                }
            }
        }
        Ok(out)
    }
}

fn expand_macros(expr: &str, macros: &HashMap<String, String>) -> String {
    let mut out = expr.to_string();
    for (src, dst) in macros {
        out = out.replace(src, dst);
    }
    out
}

fn expand_vars(expr: &str, vars: &HashMap<String, Vec<String>>) -> (String, Vec<Vec<String>>) {
    let re_var = Regex::new(r"<(.+?)>").unwrap();
    let mut used = Vec::new();
    let expanded = re_var.replace_all(expr, |caps: &Captures| {
        let vals = vars.get(&caps[1]).cloned().unwrap_or_default();
        used.push(vals.clone());
        format!(
            "({})",
            vals.iter()
                .map(|val| regex::escape(val))
                .collect::<Vec<_>>()
                .join("|")
        )
    });
    (expanded.to_string(), used)
}

fn expand_lookaround(expr: &str) -> Result<(String, Option<String>, Option<String>, isize, isize)> {
    let (pos, neg_a, neg_a_width) = expand_lookahead(expr)?;
    let (pos, neg_b, neg_b_width) = expand_lookbehind(&pos)?;
    if Regex::new(r"\{[^}]+\}").unwrap().is_match(&pos) {
        return Err(Error::InvalidRule(format!(
            "zero-width group in middle: {expr}"
        )));
    }
    Ok((pos, neg_a, neg_b, neg_a_width, neg_b_width))
}

fn expand_lookahead(expr: &str) -> Result<(String, Option<String>, isize)> {
    let re = Regex::new(r"(?s)(?:\{([^}]+)\})?(\$*)$").unwrap();
    let caps = re.captures(expr).unwrap();
    let whole = caps.get(0).unwrap();
    let other = &expr[..whole.start()];
    let edge = caps
        .get(2)
        .map(|m| no_capture(m.as_str()))
        .unwrap_or_default();
    let look = caps
        .get(1)
        .map(|m| no_capture(m.as_str()))
        .unwrap_or_default();
    let (look, neg, width, ok) = dissolve_lookaround(&look, edge.is_empty(), true);
    if !ok {
        return Ok((".^^".to_string(), None, 0));
    }
    Ok((format!("{other}({look})({edge})"), neg, width))
}

fn expand_lookbehind(expr: &str) -> Result<(String, Option<String>, isize)> {
    let re = Regex::new(r"(?s)^(\^*)(?:\{([^}]+)\})?").unwrap();
    let caps = re.captures(expr).unwrap();
    let whole = caps.get(0).unwrap();
    let other = &expr[whole.end()..];
    let edge = caps
        .get(1)
        .map(|m| no_capture(m.as_str()))
        .unwrap_or_default();
    let look = caps
        .get(2)
        .map(|m| no_capture(m.as_str()))
        .unwrap_or_default();
    let (look, neg, width, ok) = dissolve_lookaround(&look, edge.is_empty(), false);
    if !ok {
        return Ok((".^^".to_string(), None, 0));
    }
    Ok((format!("({edge})({look}){other}"), neg, width))
}

fn dissolve_lookaround(
    look: &str,
    no_edge: bool,
    ahead: bool,
) -> (String, Option<String>, isize, bool) {
    if look.is_empty() {
        return (String::new(), None, 0, true);
    }
    if let Some(negative) = look.strip_prefix('~') {
        if !no_edge {
            return (String::new(), None, 0, true);
        }
        let expr = if ahead {
            format!("^({negative})")
        } else {
            format!("({negative})$")
        };
        let width = regexp_max_width(negative);
        return (String::new(), Some(expr), width, true);
    }
    if !no_edge {
        return (String::new(), None, 0, false);
    }
    (look.to_string(), None, 0, true)
}

fn expand_edges(expr: &str) -> String {
    let mut out = Regex::new(r"\^+")
        .unwrap()
        .replace_all(expr, |caps: &Captures| {
            if caps.get(0).unwrap().as_str() == "^" {
                r"(?:^|\s+|\{\})".to_string()
            } else {
                "^".to_string()
            }
        })
        .to_string();
    out = Regex::new(r"\$+")
        .unwrap()
        .replace_all(&out, |caps: &Captures| {
            if caps.get(0).unwrap().as_str() == "$" {
                r"(?:$|\s+|\{\})".to_string()
            } else {
                "$".to_string()
            }
        })
        .to_string();
    out
}

fn no_capture(expr: &str) -> String {
    expr.replace('(', "(?:")
}

fn escape_zero_width_space(expr: &str) -> String {
    expr.replace("{}", r"\{\}")
}

fn compile_optional(expr: Option<&str>) -> Result<Option<Regex>> {
    expr.filter(|s| !s.is_empty())
        .map(|s| Regex::new(s).map_err(|err| Error::InvalidRule(err.to_string())))
        .transpose()
}

fn regexp_max_width(expr: &str) -> isize {
    if expr.contains('*') || expr.contains('+') {
        -1
    } else {
        expr.chars()
            .filter(|ch| !matches!(ch, '(' | ')' | '|' | '?' | ':'))
            .count() as isize
    }
}

fn capture_positions(caps: &Captures, offset: usize) -> Vec<Option<(usize, usize)>> {
    (0..caps.len())
        .map(|i| caps.get(i).map(|m| (m.start() + offset, m.end() + offset)))
        .collect()
}

fn substr(s: &str, start: usize, stop: usize) -> &str {
    if start >= s.len() || stop > s.len() || stop <= start {
        ""
    } else {
        &s[start..stop]
    }
}

#[derive(Clone, Debug)]
struct Replacement {
    start: usize,
    stop: usize,
    word: String,
}

impl Replacement {
    fn new(start: usize, stop: usize, word: &str) -> Self {
        Self {
            start,
            stop,
            word: word.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct Subword {
    word: String,
    level: u8,
}

impl Subword {
    fn new(word: &str, level: u8) -> Self {
        Self {
            word: word.to_string(),
            level,
        }
    }
}

struct Replacer {
    word: String,
    repls: Vec<Replacement>,
    levels: Vec<u8>,
    next_level: u8,
}

impl Replacer {
    fn new(word: &str, prev_level: u8, next_level: u8) -> Self {
        Self {
            word: word.to_string(),
            repls: Vec::new(),
            levels: vec![prev_level; word.len()],
            next_level,
        }
    }

    fn with_levels(word: &str, levels: Vec<u8>, next_level: u8) -> Self {
        Self {
            word: word.to_string(),
            repls: Vec::new(),
            levels,
            next_level,
        }
    }

    fn replace(&mut self, start: usize, stop: usize, word: &str) {
        self.repls.push(Replacement::new(start, stop, word));
    }

    fn replace_by(&mut self, repls: Vec<Replacement>) {
        self.repls.extend(repls);
    }

    fn commit(&mut self) {
        let mut out = String::new();
        let mut levels = Vec::new();
        let mut offset = 0;
        self.repls.sort_by_key(|repl| repl.start);
        for repl in self.repls.drain(..) {
            if repl.start < offset || repl.stop > self.word.len() {
                continue;
            }
            out.push_str(&self.word[offset..repl.start]);
            levels.extend_from_slice(&self.levels[offset..repl.start]);
            out.push_str(&repl.word);
            levels.extend(std::iter::repeat_n(self.next_level, repl.word.len()));
            offset = repl.stop;
        }
        out.push_str(&self.word[offset..]);
        levels.extend_from_slice(&self.levels[offset..]);
        self.word = out;
        self.levels = levels;
    }

    fn string(&mut self) -> String {
        self.commit();
        self.word.clone()
    }

    fn subwords(&mut self) -> Vec<Subword> {
        self.commit();
        let mut out = Vec::new();
        if self.word.is_empty() {
            return out;
        }
        let mut current = self.levels[0];
        let mut start = 0;
        for (idx, _) in self.word.char_indices() {
            if self.levels[idx] != current {
                out.push(Subword::new(&self.word[start..idx], current));
                current = self.levels[idx];
                start = idx;
            }
        }
        out.push(Subword::new(&self.word[start..], current));
        out
    }

    fn into_parts(mut self) -> (String, Vec<u8>) {
        self.commit();
        (self.word, self.levels)
    }
}

#[derive(Default)]
struct SubwordBuilder {
    subwords: Vec<Subword>,
}

impl SubwordBuilder {
    fn write(&mut self, subwords: Vec<Subword>) {
        self.subwords.extend(subwords);
    }

    fn subwords(self) -> Vec<Subword> {
        let mut out: Vec<Subword> = Vec::new();
        for sw in self.subwords {
            if let Some(last) = out.last_mut()
                && last.level == sw.level
            {
                last.word.push_str(&sw.word);
                continue;
            }
            out.push(sw);
        }
        out
    }
}

#[derive(Clone)]
enum Script {
    Latn,
    Cyrl,
    Geor,
    Grek,
    Hrkt,
}

impl Script {
    fn from_name(name: &str) -> Self {
        match name {
            "Cyrl" => Self::Cyrl,
            "Geor" => Self::Geor,
            "Grek" => Self::Grek,
            "Hrkt" => Self::Hrkt,
            _ => Self::Latn,
        }
    }

    fn is(&self, ch: char) -> bool {
        match self {
            Self::Latn => {
                matches!(ch as u32, 0x0041..=0x007a | 0x00c0..=0x02af | 0x1e00..=0x1eff)
            }
            Self::Cyrl => {
                matches!(ch as u32, 0x0300..=0x036f | 0x0400..=0x052f | 0x2de0..=0x2dff | 0xa640..=0xa69f)
            }
            Self::Geor => matches!(ch as u32, 0x10a0..=0x10ff | 0x1c90..=0x1cbf | 0x2d00..=0x2d2f),
            Self::Grek => matches!(ch as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff),
            Self::Hrkt => ch == 'ー' || matches!(ch as u32, 0x3040..=0x30ff),
        }
    }

    fn normalize(&self, ch: char) -> char {
        match self {
            Self::Latn => ch
                .to_string()
                .nfd()
                .next()
                .unwrap_or(ch)
                .to_lowercase()
                .next()
                .unwrap_or(ch),
            Self::Hrkt if matches!(ch as u32, 0x3040..=0x309f) => {
                char::from_u32(ch as u32 + 96).unwrap_or(ch)
            }
            Self::Geor => ch,
            _ => ch.to_lowercase().next().unwrap_or(ch),
        }
    }

    fn localize_punct(&self, punct: char) -> String {
        if matches!(self, Self::Hrkt) {
            match punct {
                '。' => ". ".to_string(),
                '、' => ", ".to_string(),
                '：' => ": ".to_string(),
                '！' => "! ".to_string(),
                '？' => "? ".to_string(),
                '〜' => "~".to_string(),
                '「' => " '".to_string(),
                '」' => "' ".to_string(),
                '『' => " \"".to_string(),
                '』' => "\" ".to_string(),
                _ => punct.to_string(),
            }
        } else {
            punct.to_string()
        }
    }
}

fn is_space_only(ch: char) -> bool {
    ch.is_whitespace()
}

fn is_rule_separator(ch: char) -> bool {
    matches!(ch, '-' | '\'' | '’')
}

fn is_punct(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch,
            '。' | '、' | '：' | '！' | '？' | '〜' | '「' | '」' | '『' | '』'
        )
}

#[derive(Clone)]
struct StringMapReplacer {
    map: HashMap<String, String>,
    key_lengths: Vec<usize>,
}

impl StringMapReplacer {
    fn new(map: HashMap<String, String>) -> Self {
        let mut key_lengths = map.keys().map(String::len).collect::<Vec<_>>();
        key_lengths.sort_unstable();
        key_lengths.dedup();
        key_lengths.reverse();
        Self { map, key_lengths }
    }

    fn replace(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        let mut pos = 0;
        while pos < input.len() {
            let mut matched = None;
            for len in &self.key_lengths {
                let end = pos + len;
                if end > input.len() || !input.is_char_boundary(end) {
                    continue;
                }
                let candidate = &input[pos..end];
                if let Some(replacement) = self.map.get(candidate) {
                    matched = Some((end, replacement.as_str()));
                    break;
                }
            }
            if let Some((end, replacement)) = matched {
                out.push_str(replacement);
                pos = end;
            } else {
                let ch = input[pos..].chars().next().unwrap();
                out.push(ch);
                pos += ch.len_utf8();
            }
        }
        out
    }
}

static CYRILLIC_REPLACERS: Lazy<Result<HashMap<String, StringMapReplacer>>> = Lazy::new(|| {
    let root: Value =
        serde_json::from_str(CYRILLIC_DICT).map_err(|err| Error::InvalidSpec(err.to_string()))?;
    let Some(countries) = root.as_object() else {
        return Err(Error::InvalidSpec(
            "invalid cyrillic dictionary".to_string(),
        ));
    };
    let mut out = HashMap::with_capacity(countries.len());
    for (country, value) in countries {
        let Some(to_cyrillic) = value.get("tocyrillic").and_then(Value::as_object) else {
            continue;
        };
        let mut map = HashMap::with_capacity(to_cyrillic.len());
        for (from, to) in to_cyrillic {
            if let Some(to) = to.as_str() {
                map.insert(from.clone(), to.to_string());
            }
        }
        out.insert(country.clone(), StringMapReplacer::new(map));
    }
    Ok(out)
});

fn transliterate_pinyin(word: &str) -> String {
    let word = word.nfc().collect::<String>();
    let mut out = String::new();
    for ch in word.chars() {
        if let Some(py) = ch.to_pinyin() {
            if !out.is_empty() {
                out.push('\u{200b}');
            }
            out.push_str(py.plain());
        } else {
            out.push(ch);
        }
    }
    out
}

static FURIGANA_TOKENIZER: Lazy<Result<std::sync::Mutex<Tokenizer>>> = Lazy::new(|| {
    let dictionary =
        load_dictionary("embedded://ipadic").map_err(|err| Error::InvalidSpec(err.to_string()))?;
    let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
    Ok(std::sync::Mutex::new(Tokenizer::new(segmenter)))
});

static ENGLISH_G2P: Lazy<std::sync::Mutex<misaki_rs::G2P>> =
    Lazy::new(|| std::sync::Mutex::new(misaki_rs::G2P::new(misaki_rs::Language::EnglishUS)));

fn transliterate_english_phoneme(word: &str) -> Result<String> {
    let mut out = String::new();
    let mut segment = String::new();
    for ch in word.chars() {
        if ch.is_ascii_alphanumeric() || ch == '\'' {
            segment.push(ch);
        } else {
            flush_english_segment(&mut out, &mut segment)?;
            out.push(ch);
        }
    }
    flush_english_segment(&mut out, &mut segment)?;
    Ok(out)
}

fn flush_english_segment(out: &mut String, segment: &mut String) -> Result<()> {
    if !segment.is_empty() {
        out.push_str(&english_segment_to_hangul(segment)?);
        segment.clear();
    }
    Ok(())
}

fn english_segment_to_hangul(word: &str) -> Result<String> {
    let g2p = ENGLISH_G2P.lock().unwrap();
    let (phonemes, _) = g2p
        .g2p(word)
        .map_err(|err| Error::InvalidSpec(err.to_string()))?;
    Ok(english_phonemes_to_hangul(&phonemes))
}

fn english_phonemes_to_hangul(phonemes: &str) -> String {
    let mut out = String::new();
    let mut buf = String::new();
    for ch in phonemes.chars() {
        if ch.is_whitespace() {
            flush_english_word(&mut out, &mut buf);
            out.push(ch);
        } else if is_punct(ch) {
            flush_english_word(&mut out, &mut buf);
            out.push(ch);
        } else {
            buf.push(ch);
        }
    }
    flush_english_word(&mut out, &mut buf);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn flush_english_word(out: &mut String, buf: &mut String) {
    if !buf.is_empty() {
        out.push_str(&english_phoneme_word_to_hangul(buf));
        buf.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnglishUnit {
    Consonant(char),
    Vowel(char),
    Letter(&'static str),
}

fn english_phoneme_word_to_hangul(word: &str) -> String {
    let units = english_units(word);
    if units
        .iter()
        .all(|unit| matches!(unit, EnglishUnit::Letter(_)))
    {
        return units
            .iter()
            .filter_map(|unit| match unit {
                EnglishUnit::Letter(hangul) => Some(*hangul),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
    }

    let mut out = String::new();
    let mut i = 0;
    let mut pending = Vec::new();
    while i < units.len() {
        match units[i] {
            EnglishUnit::Consonant(c) => {
                pending.push(c);
                i += 1;
            }
            EnglishUnit::Letter(hangul) => {
                out.push_str(hangul);
                i += 1;
            }
            EnglishUnit::Vowel(vowel) => {
                let onset = pending.pop().unwrap_or('ㅇ');
                let (onset, vowel) = english_onset_vowel(onset, vowel);
                if !pending.is_empty() {
                    out.push_str(&render_english_consonants(&pending));
                    pending.clear();
                }
                let mut j = i + 1;
                let mut after = Vec::new();
                while j < units.len() {
                    if let EnglishUnit::Consonant(c) = units[j] {
                        after.push(c);
                        j += 1;
                    } else {
                        break;
                    }
                }
                let next_is_vowel = j < units.len() && matches!(units[j], EnglishUnit::Vowel(_));
                if next_is_vowel {
                    if after == ['ㄹ'] {
                        out.push_str(&compose_hangul(&format!("{onset}{vowel}-ㄹ")));
                        pending.push('ㄹ');
                    } else {
                        out.push_str(&compose_hangul(&format!("{onset}{vowel}")));
                        pending = after;
                    }
                } else if let Some((lead, tail)) = final_blend(&after) {
                    out.push_str(&compose_hangul(&format!("{onset}{vowel}")));
                    out.push_str(&compose_hangul(&format!("{lead}ㅡ-{tail}")));
                    pending.clear();
                } else {
                    let (tail, rest) = split_final_cluster(&after);
                    if let Some(tail) = tail {
                        out.push_str(&compose_hangul(&format!("{onset}{vowel}-{tail}")));
                    } else {
                        out.push_str(&compose_hangul(&format!("{onset}{vowel}")));
                    }
                    if !rest.is_empty() {
                        out.push_str(&render_english_consonants(rest));
                    }
                    pending.clear();
                }
                i = j;
            }
        }
    }
    if !pending.is_empty() {
        out.push_str(&render_english_consonants(&pending));
    }
    out
}

fn english_units(word: &str) -> Vec<EnglishUnit> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < word.len() {
        let rest = &word[i..];
        if let Some(skip) = english_skip_len(rest) {
            i += skip;
            continue;
        }
        if rest.starts_with("o‍ʊ") || rest.starts_with("oʊ") {
            out.push(EnglishUnit::Vowel('ㅗ'));
            i += if rest.starts_with("o‍ʊ") {
                "o‍ʊ".len()
            } else {
                "oʊ".len()
            };
        } else if rest.starts_with("e‍ɪ") || rest.starts_with("eɪ") {
            out.push(EnglishUnit::Letter("에이"));
            i += if rest.starts_with("e‍ɪ") {
                "e‍ɪ".len()
            } else {
                "eɪ".len()
            };
        } else if rest.starts_with("a‍ɪ") || rest.starts_with("aɪ") {
            out.push(EnglishUnit::Letter("아이"));
            i += if rest.starts_with("a‍ɪ") {
                "a‍ɪ".len()
            } else {
                "aɪ".len()
            };
        } else if rest.starts_with("a‍ʊ") || rest.starts_with("aʊ") {
            out.push(EnglishUnit::Letter("아우"));
            i += if rest.starts_with("a‍ʊ") {
                "a‍ʊ".len()
            } else {
                "aʊ".len()
            };
        } else if rest.starts_with("ɔ‍ɪ") || rest.starts_with("ɔɪ") {
            out.push(EnglishUnit::Letter("오이"));
            i += if rest.starts_with("ɔ‍ɪ") {
                "ɔ‍ɪ".len()
            } else {
                "ɔɪ".len()
            };
        } else if rest.starts_with("tʃ") {
            out.push(EnglishUnit::Consonant('ㅊ'));
            i += "tʃ".len();
        } else if rest.starts_with("dʒ") {
            out.push(EnglishUnit::Consonant('ㅈ'));
            i += "dʒ".len();
        } else {
            let ch = rest.chars().next().unwrap();
            if let Some(unit) = english_unit(ch) {
                out.push(unit);
            }
            i += ch.len_utf8();
        }
    }
    collapse_syllabic_schwa_l(out)
}

fn collapse_syllabic_schwa_l(units: Vec<EnglishUnit>) -> Vec<EnglishUnit> {
    let mut out = Vec::with_capacity(units.len());
    for (i, unit) in units.iter().copied().enumerate() {
        let syllabic_l = matches!(unit, EnglishUnit::Vowel('ㅔ'))
            && i > 0
            && i + 1 == units.len() - 1
            && matches!(units[i - 1], EnglishUnit::Consonant(_))
            && matches!(units[i + 1], EnglishUnit::Consonant('ㄹ'));
        if !syllabic_l {
            out.push(unit);
        }
    }
    out
}

fn english_skip_len(rest: &str) -> Option<usize> {
    rest.chars().next().and_then(|ch| {
        if matches!(
            ch,
            'ˈ' | 'ˌ' | 'ː' | '\u{200d}' | '\u{200c}' | '\u{0361}' | 'ᵊ'
        ) {
            Some(ch.len_utf8())
        } else {
            None
        }
    })
}

fn english_unit(ch: char) -> Option<EnglishUnit> {
    Some(match ch {
        'A' => EnglishUnit::Letter("에이"),
        'B' => EnglishUnit::Letter("비"),
        'C' => EnglishUnit::Letter("시"),
        'D' => EnglishUnit::Letter("디"),
        'E' => EnglishUnit::Letter("이"),
        'F' => EnglishUnit::Letter("에프"),
        'G' => EnglishUnit::Letter("지"),
        'H' => EnglishUnit::Letter("에이치"),
        'I' => EnglishUnit::Letter("아이"),
        'J' => EnglishUnit::Letter("제이"),
        'K' => EnglishUnit::Letter("케이"),
        'L' => EnglishUnit::Letter("엘"),
        'M' => EnglishUnit::Letter("엠"),
        'N' => EnglishUnit::Letter("엔"),
        'O' => EnglishUnit::Letter("오"),
        'P' => EnglishUnit::Letter("피"),
        'Q' => EnglishUnit::Letter("큐"),
        'R' => EnglishUnit::Letter("아르"),
        'S' => EnglishUnit::Letter("에스"),
        'T' => EnglishUnit::Letter("티"),
        'U' => EnglishUnit::Letter("유"),
        'V' => EnglishUnit::Letter("브이"),
        'W' => EnglishUnit::Letter("더블유"),
        'X' => EnglishUnit::Letter("엑스"),
        'Y' => EnglishUnit::Letter("와이"),
        'Z' => EnglishUnit::Letter("지"),
        'æ' => EnglishUnit::Vowel('ㅐ'),
        'ɛ' | 'ə' => EnglishUnit::Vowel('ㅔ'),
        'ɜ' | 'ʌ' | 'ɔ' => EnglishUnit::Vowel('ㅓ'),
        'ɑ' | 'a' => EnglishUnit::Vowel('ㅏ'),
        'i' | 'ɪ' => EnglishUnit::Vowel('ㅣ'),
        'u' | 'ʊ' => EnglishUnit::Vowel('ㅜ'),
        'o' => EnglishUnit::Vowel('ㅗ'),
        'ɡ' | 'g' => EnglishUnit::Consonant('ㄱ'),
        'k' => EnglishUnit::Consonant('ㅋ'),
        't' => EnglishUnit::Consonant('ㅌ'),
        'd' => EnglishUnit::Consonant('ㄷ'),
        'p' => EnglishUnit::Consonant('ㅍ'),
        'b' => EnglishUnit::Consonant('ㅂ'),
        'f' | 'v' => EnglishUnit::Consonant('ㅍ'),
        's' | 'θ' => EnglishUnit::Consonant('ㅅ'),
        'z' | 'ð' => EnglishUnit::Consonant('ㅈ'),
        'ʃ' => EnglishUnit::Consonant('ㅅ'),
        'ʒ' => EnglishUnit::Consonant('ㅈ'),
        'h' => EnglishUnit::Consonant('ㅎ'),
        'm' => EnglishUnit::Consonant('ㅁ'),
        'n' => EnglishUnit::Consonant('ㄴ'),
        'ŋ' => EnglishUnit::Consonant('ㅇ'),
        'l' | 'ɫ' | 'r' | 'ɹ' => EnglishUnit::Consonant('ㄹ'),
        'w' => EnglishUnit::Consonant('W'),
        'j' => EnglishUnit::Consonant('Y'),
        _ => return None,
    })
}

fn english_onset_vowel(onset: char, vowel: char) -> (char, char) {
    match (onset, vowel) {
        ('W', 'ㅏ') => ('ㅇ', 'ㅘ'),
        ('W', 'ㅐ' | 'ㅔ') => ('ㅇ', 'ㅞ'),
        ('W', 'ㅓ') => ('ㅇ', 'ㅝ'),
        ('W', 'ㅣ') => ('ㅇ', 'ㅟ'),
        ('W', 'ㅜ') => ('ㅇ', 'ㅜ'),
        ('Y', 'ㅏ') => ('ㅇ', 'ㅑ'),
        ('Y', 'ㅓ' | 'ㅔ') => ('ㅇ', 'ㅖ'),
        ('Y', 'ㅗ') => ('ㅇ', 'ㅛ'),
        ('Y', 'ㅜ') => ('ㅇ', 'ㅠ'),
        ('Y', 'ㅣ') => ('ㅇ', 'ㅣ'),
        (other, vowel) => (other, vowel),
    }
}

fn final_blend(cluster: &[char]) -> Option<(char, char)> {
    match cluster {
        ['ㅍ', 'ㄹ'] => Some(('ㅍ', 'ㄹ')),
        ['ㅂ', 'ㄹ'] => Some(('ㅂ', 'ㄹ')),
        ['ㄱ', 'ㄹ'] => Some(('ㄱ', 'ㄹ')),
        ['ㅋ', 'ㄹ'] => Some(('ㅋ', 'ㄹ')),
        _ => None,
    }
}

fn split_final_cluster(cluster: &[char]) -> (Option<char>, &[char]) {
    if let Some((first, rest)) = cluster.split_first()
        && is_english_tail(*first)
    {
        (Some(english_tail(*first)), rest)
    } else {
        (None, cluster)
    }
}

fn is_english_tail(ch: char) -> bool {
    matches!(
        ch,
        'ㄱ' | 'ㅋ' | 'ㄴ' | 'ㄷ' | 'ㄹ' | 'ㅁ' | 'ㅂ' | 'ㅍ' | 'ㅅ' | 'ㅇ'
    )
}

fn english_tail(ch: char) -> char {
    match ch {
        'ㅋ' => 'ㄱ',
        'ㅍ' => 'ㅂ',
        'ㅌ' | 'ㄷ' => 'ㅅ',
        other => other,
    }
}

fn render_english_consonants(consonants: &[char]) -> String {
    consonants
        .iter()
        .map(|ch| match ch {
            'W' => "우".to_string(),
            'Y' => "이".to_string(),
            ch => compose_hangul(&format!("{ch}ㅡ")),
        })
        .collect()
}

fn transliterate_furigana(word: &str, trim_terminal_long_vowels: bool) -> Result<String> {
    let word = repeat_kana(&word.nfc().collect::<String>());
    let mut out = String::new();
    let mut segment = String::new();
    for ch in word.chars() {
        if ch.is_whitespace() {
            if !segment.is_empty() {
                out.push_str(&transliterate_furigana_segment(
                    &segment,
                    trim_terminal_long_vowels,
                )?);
                segment.clear();
            }
            out.push(ch);
        } else {
            segment.push(ch);
        }
    }
    if !segment.is_empty() {
        out.push_str(&transliterate_furigana_segment(
            &segment,
            trim_terminal_long_vowels,
        )?);
    }
    Ok(out)
}

fn repeat_kana(word: &str) -> String {
    let mut out = String::new();
    let mut last_kana = None;
    for ch in word.chars() {
        let repeated = match ch {
            'ゝ' | 'ヽ' => last_kana,
            'ゞ' | 'ヾ' => last_kana.and_then(voiced_kana).or(last_kana),
            _ => None,
        };
        if let Some(rep) = repeated {
            out.push(rep);
            last_kana = Some(rep);
        } else {
            out.push(ch);
            if is_kana(ch) {
                last_kana = Some(ch);
            }
        }
    }
    out
}

fn is_kana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30ff)
}

fn voiced_kana(ch: char) -> Option<char> {
    Some(match ch {
        'か' => 'が',
        'き' => 'ぎ',
        'く' => 'ぐ',
        'け' => 'げ',
        'こ' => 'ご',
        'さ' => 'ざ',
        'し' => 'じ',
        'す' => 'ず',
        'せ' => 'ぜ',
        'そ' => 'ぞ',
        'た' => 'だ',
        'ち' => 'ぢ',
        'つ' => 'づ',
        'て' => 'で',
        'と' => 'ど',
        'は' => 'ば',
        'ひ' => 'び',
        'ふ' => 'ぶ',
        'へ' => 'べ',
        'ほ' => 'ぼ',
        'カ' => 'ガ',
        'キ' => 'ギ',
        'ク' => 'グ',
        'ケ' => 'ゲ',
        'コ' => 'ゴ',
        'サ' => 'ザ',
        'シ' => 'ジ',
        'ス' => 'ズ',
        'セ' => 'ゼ',
        'ソ' => 'ゾ',
        'タ' => 'ダ',
        'チ' => 'ヂ',
        'ツ' => 'ヅ',
        'テ' => 'デ',
        'ト' => 'ド',
        'ハ' => 'バ',
        'ヒ' => 'ビ',
        'フ' => 'ブ',
        'ヘ' => 'ベ',
        'ホ' => 'ボ',
        _ => return None,
    })
}

fn transliterate_furigana_segment(word: &str, trim_terminal_long_vowels: bool) -> Result<String> {
    let tokenizer = FURIGANA_TOKENIZER
        .as_ref()
        .map_err(|err| Error::InvalidSpec(err.to_string()))?;
    let guard = tokenizer.lock().unwrap();
    let mut tokens = guard
        .tokenize(word)
        .map_err(|err| Error::InvalidSpec(err.to_string()))?;
    let mut out = String::new();
    let mut previous_was_family_name = false;
    for token in tokens.iter_mut() {
        let surface = token.surface.as_ref().to_string();
        let details = token.details();
        let is_person_name = details.get(2) == Some(&"人名");
        let is_proper_noun =
            details.first() == Some(&"名詞") && details.get(1) == Some(&"固有名詞");
        let is_family_name = is_person_name && details.get(3) == Some(&"姓");
        let is_given_name = is_person_name && details.get(3) == Some(&"名");
        if !out.is_empty() {
            if previous_was_family_name && (is_given_name || is_proper_noun) {
                out.push(' ');
            }
        }
        let reading = details
            .get(8)
            .or_else(|| details.get(7))
            .copied()
            .filter(|reading| !reading.is_empty() && *reading != "*")
            .unwrap_or(&surface);
        let reading = if trim_terminal_long_vowels {
            trim_terminal_katakana_long_vowel(reading)
        } else {
            reading.to_string()
        };
        if details.first() == Some(&"助詞") && matches!(surface.as_str(), "は" | "へ") {
            out.push_str(&surface);
        } else if details.first() == Some(&"助動詞")
            && surface == "う"
            && out.chars().last().is_some_and(is_katakana_o_row)
        {
        } else if details.first() == Some(&"助動詞") && surface.ends_with("しい") {
            out.push_str(reading.strip_suffix('イ').unwrap_or(&reading));
        } else {
            out.push_str(&reading);
        }
        previous_was_family_name = is_family_name;
    }
    Ok(out)
}

fn trim_terminal_katakana_long_vowel(reading: &str) -> String {
    if reading.ends_with("ュウ") {
        reading.trim_end_matches('ウ').to_string()
    } else if reading.ends_with("シィ") {
        reading.trim_end_matches('ィ').to_string()
    } else {
        reading.to_string()
    }
}

fn is_katakana_o_row(ch: char) -> bool {
    matches!(
        ch,
        'オ' | 'コ'
            | 'ゴ'
            | 'ソ'
            | 'ゾ'
            | 'ト'
            | 'ド'
            | 'ノ'
            | 'ホ'
            | 'ボ'
            | 'ポ'
            | 'モ'
            | 'ヨ'
            | 'ョ'
            | 'ロ'
            | 'ヲ'
    )
}

fn transliterate_cyrillic(country: &str, word: &str) -> Result<String> {
    let replacers = CYRILLIC_REPLACERS
        .as_ref()
        .map_err(|err| Error::InvalidSpec(err.to_string()))?;
    let Some(replacer) = replacers.get(country) else {
        return Err(Error::TranslitNotAvailable(format!("cyrillic[{country}]")));
    };
    Ok(replacer.replace(word))
}

fn syllabify(subwords: &[Subword]) -> String {
    let mut out = String::new();
    let mut jamo = String::new();
    for sw in subwords {
        if sw.level == 0 {
            out.push_str(&compose_hangul(&jamo));
            jamo.clear();
            out.push_str(&sw.word);
        } else {
            jamo.push_str(&sw.word);
        }
    }
    out.push_str(&compose_hangul(&jamo));
    out
}

fn compose_hangul(word: &str) -> String {
    let mut out = String::new();
    let mut lmt = [None; 3];
    let mut prev_score = -1;
    let mut chars = word.chars().peekable();
    while let Some(mut ch) = chars.next() {
        let mut is_tail = false;
        if ch == '-' {
            is_tail = true;
            let Some(next) = chars.next() else {
                continue;
            };
            ch = next;
        }
        let score = if is_vowel(ch) {
            1
        } else if is_jaeum(ch) {
            if is_tail { 2 } else { 0 }
        } else if is_hangul_syllable(ch) {
            flush_syllable(&mut out, &mut lmt);
            let (lead, vowel, tail) = decompose_syllable(ch);
            lmt = [Some(lead), Some(vowel), tail];
            prev_score = if tail.is_some() { 2 } else { 1 };
            continue;
        } else {
            if prev_score != -1 {
                flush_syllable(&mut out, &mut lmt);
            }
            out.push(ch);
            prev_score = -1;
            continue;
        };
        if score <= prev_score {
            flush_syllable(&mut out, &mut lmt);
        }
        lmt[score as usize] = Some(ch);
        prev_score = score;
    }
    if prev_score != -1 {
        flush_syllable(&mut out, &mut lmt);
    }
    out
}

fn flush_syllable(out: &mut String, lmt: &mut [Option<char>; 3]) {
    if lmt.iter().all(Option::is_none) {
        return;
    }
    let lead = lmt[0].unwrap_or('ㅇ');
    let vowel = lmt[1].unwrap_or('ㅡ');
    let tail = lmt[2];
    if let (Some(l), Some(v)) = (lead_index(lead), vowel_index(vowel)) {
        let t = tail.and_then(tail_index).unwrap_or(0);
        let code = 0xac00 + ((l * 21 + v) * 28 + t) as u32;
        out.push(char::from_u32(code).unwrap());
    } else {
        if let Some(ch) = lmt[0] {
            out.push(ch);
        }
        if let Some(ch) = lmt[1] {
            out.push(ch);
        }
        if let Some(ch) = lmt[2] {
            out.push(ch);
        }
    }
    *lmt = [None; 3];
}

fn is_hangul_syllable(ch: char) -> bool {
    matches!(ch as u32, 0xac00..=0xd7a3)
}

fn decompose_syllable(ch: char) -> (char, char, Option<char>) {
    const LEADS: [char; 19] = [
        'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ',
        'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
    ];
    const VOWELS: [char; 21] = [
        'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ',
        'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
    ];
    const TAILS: [Option<char>; 28] = [
        None,
        Some('ㄱ'),
        Some('ㄲ'),
        Some('ㄳ'),
        Some('ㄴ'),
        Some('ㄵ'),
        Some('ㄶ'),
        Some('ㄷ'),
        Some('ㄹ'),
        Some('ㄺ'),
        Some('ㄻ'),
        Some('ㄼ'),
        Some('ㄽ'),
        Some('ㄾ'),
        Some('ㄿ'),
        Some('ㅀ'),
        Some('ㅁ'),
        Some('ㅂ'),
        Some('ㅄ'),
        Some('ㅅ'),
        Some('ㅆ'),
        Some('ㅇ'),
        Some('ㅈ'),
        Some('ㅊ'),
        Some('ㅋ'),
        Some('ㅌ'),
        Some('ㅍ'),
        Some('ㅎ'),
    ];
    let n = ch as u32 - 0xac00;
    let l = (n / 588) as usize;
    let v = ((n % 588) / 28) as usize;
    let t = (n % 28) as usize;
    (LEADS[l], VOWELS[v], TAILS[t])
}

fn is_jaeum(ch: char) -> bool {
    lead_index(ch).is_some() || tail_index(ch).is_some()
}

fn is_vowel(ch: char) -> bool {
    vowel_index(ch).is_some()
}

fn lead_index(ch: char) -> Option<usize> {
    [
        'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ',
        'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
    ]
    .iter()
    .position(|candidate| *candidate == ch)
}

fn vowel_index(ch: char) -> Option<usize> {
    [
        'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ',
        'ㅞ', 'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
    ]
    .iter()
    .position(|candidate| *candidate == ch)
}

fn tail_index(ch: char) -> Option<usize> {
    [
        None,
        Some('ㄱ'),
        Some('ㄲ'),
        Some('ㄳ'),
        Some('ㄴ'),
        Some('ㄵ'),
        Some('ㄶ'),
        Some('ㄷ'),
        Some('ㄹ'),
        Some('ㄺ'),
        Some('ㄻ'),
        Some('ㄼ'),
        Some('ㄽ'),
        Some('ㄾ'),
        Some('ㄿ'),
        Some('ㅀ'),
        Some('ㅁ'),
        Some('ㅂ'),
        Some('ㅄ'),
        Some('ㅅ'),
        Some('ㅆ'),
        Some('ㅇ'),
        Some('ㅈ'),
        Some('ㅊ'),
        Some('ㅋ'),
        Some('ㅌ'),
        Some('ㅍ'),
        Some('ㅎ'),
    ]
    .iter()
    .position(|candidate| *candidate == Some(ch))
}

#[derive(Clone)]
struct Pair {
    left: String,
    right: Vec<String>,
}

#[derive(Clone)]
enum Section {
    Dict(Vec<Pair>),
    List(Vec<Pair>),
}

fn parse_hsl(src: &str) -> Result<HashMap<String, Section>> {
    let mut sections = HashMap::new();
    let mut current = String::new();
    for raw in src.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(section) = line.strip_suffix(':') {
            current = section.trim().to_string();
            continue;
        }
        if current.is_empty() {
            return Err(Error::InvalidSpec("pair found outside section".to_string()));
        }
        if let Some((left, right)) = split_once_operator(line, "->") {
            let pair = Pair {
                left: parse_value(left)?,
                right: parse_values(right)?,
            };
            match sections
                .entry(current.clone())
                .or_insert_with(|| Section::List(Vec::new()))
            {
                Section::List(pairs) => pairs.push(pair),
                Section::Dict(_) => {
                    return Err(Error::InvalidSpec(format!("mixed section: {current}")));
                }
            }
        } else if let Some((left, right)) = split_once_operator(line, "=") {
            let pair = Pair {
                left: parse_value(left)?,
                right: parse_values(right)?,
            };
            match sections
                .entry(current.clone())
                .or_insert_with(|| Section::Dict(Vec::new()))
            {
                Section::Dict(pairs) => pairs.push(pair),
                Section::List(_) => {
                    return Err(Error::InvalidSpec(format!("mixed section: {current}")));
                }
            }
        }
    }
    Ok(sections)
}

fn strip_comment(line: &str) -> &str {
    if let Some(idx) = line.find('#') {
        &line[..idx]
    } else {
        line
    }
}

fn split_once_operator<'a>(line: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    line.find(op)
        .map(|idx| (&line[..idx], &line[idx + op.len()..]))
}

fn parse_values(src: &str) -> Result<Vec<String>> {
    let mut values = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (idx, ch) in src.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            quoted = !quoted;
            continue;
        }
        if ch == ',' && !quoted {
            values.push(parse_value(&src[start..idx])?);
            start = idx + ch.len_utf8();
        }
    }
    values.push(parse_value(&src[start..])?);
    Ok(values)
}

fn parse_value(src: &str) -> Result<String> {
    let src = src.trim();
    if src.starts_with('"') {
        let mut out = String::new();
        let mut escaped = false;
        for ch in src[1..].chars() {
            if ch == '"' && !escaped {
                return Ok(out);
            }
            if ch == '\\' && !escaped {
                escaped = true;
                continue;
            }
            escaped = false;
            out.push(ch);
        }
        Err(Error::InvalidSpec(format!("unclosed string: {src}")))
    } else {
        Ok(src.to_string())
    }
}

fn dict_one_to_one(section: &Section) -> Result<HashMap<String, String>> {
    let Section::Dict(pairs) = section else {
        return Err(Error::InvalidSpec("expected dict section".to_string()));
    };
    let mut out = HashMap::new();
    for pair in pairs {
        let Some(value) = pair.right.first() else {
            continue;
        };
        if pair.right.len() != 1 {
            return Err(Error::InvalidSpec("expected single value".to_string()));
        }
        out.insert(pair.left.clone(), value.clone());
    }
    Ok(out)
}

fn dict_map(section: &Section) -> HashMap<String, Vec<String>> {
    let Section::Dict(pairs) = section else {
        return HashMap::new();
    };
    pairs
        .iter()
        .map(|pair| (pair.left.clone(), pair.right.clone()))
        .collect()
}

fn one(pairs: &[Pair], key: &str) -> String {
    pairs
        .iter()
        .find(|pair| pair.left == key)
        .and_then(|pair| pair.right.first())
        .cloned()
        .unwrap_or_default()
}

fn all(pairs: &[Pair], key: &str) -> Vec<String> {
    pairs
        .iter()
        .find(|pair| pair.left == key)
        .map(|pair| pair.right.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_basic_compat_jamo() {
        assert_eq!(compose_hangul("ㅋㅏㅍㅜㅊㅣㄴㅗ"), "카푸치노");
        assert_eq!(compose_hangul("ㅇㅏ-ㄹ"), "알");
    }

    #[test]
    fn parses_simple_hsl() {
        let parsed = parse_hsl("lang:\n id = \"x\"\ntranscribe:\n \"a\" -> \"ㅏ\"\n").unwrap();
        assert!(parsed.contains_key("lang"));
        assert!(parsed.contains_key("transcribe"));
    }

    #[test]
    fn hre_lookahead_matches_without_consuming() {
        let mut macros = HashMap::new();
        macros.insert("@".to_string(), "<vowels>".to_string());
        let mut vars = HashMap::new();
        vars.insert(
            "vowels".to_string(),
            vec!["a".to_string(), "A".to_string(), "e".to_string()],
        );
        let rule = Rule {
            from: Pattern::new("S{@}", &macros, &vars).unwrap(),
            to: RPattern::new("sY", &macros, &vars),
        };
        let repls = rule.replacements("SAmkir").unwrap();
        assert_eq!(repls.len(), 1);
        assert_eq!(repls[0].start, 0);
        assert_eq!(repls[0].stop, 1);
        assert_eq!(repls[0].word, "sY");
    }

    #[test]
    fn aze_normalization_preserves_custom_letters() {
        let spec = Spec::parse(include_str!("specs/aze.hsl")).unwrap();
        let h = Hangulizer {
            spec: Arc::new(spec),
        };
        assert_eq!(h.normalize("Şəmkir"), "şəmkir");
    }

    #[test]
    fn aze_rewrite_handles_s_before_schwa() {
        let spec = Spec::parse(include_str!("specs/aze.hsl")).unwrap();
        let h = Hangulizer {
            spec: Arc::new(spec),
        };
        let order = h
            .spec
            .rewrite
            .iter()
            .map(|rule| rule.from.expr.as_str())
            .take(8)
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["-", "ə", "ç", "ğ", "ı", "ö", "ş", "ü"]);
        let s_lookahead = h
            .spec
            .rewrite
            .iter()
            .find(|rule| rule.from.expr == "S{@}")
            .unwrap();
        assert_eq!(s_lookahead.replacements("SAmkir").unwrap().len(), 1);
        let normalized = h.normalize("Şəmkir");
        let mut word = normalized.clone();
        for rule in &h.spec.rewrite {
            let repls = rule.replacements(&word).unwrap();
            let mut rep = Replacer::new(&word, 1, 1);
            rep.replace_by(repls);
            word = rep.string();
            if rule.from.expr == "ş" {
                assert_eq!(word, "SAmkir");
            }
            if rule.from.expr == "S{@}" {
                assert_eq!(word, "sYAmkir");
                break;
            }
        }
        let rewritten = h.rewrite(h.partition(&normalized)).unwrap();
        let joined = rewritten.into_iter().map(|sw| sw.word).collect::<String>();
        assert!(joined.starts_with("sYAm"), "{joined}");
    }
}
