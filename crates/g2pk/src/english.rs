use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

static ENG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z']+").unwrap());
static LEXICON: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("file", "파일"),
        ("game", "게임"),
        ("old", "올드"),
        ("school", "스쿨"),
        ("tts", "티티에스"),
        ("asr", "에이에스알"),
    ])
});

pub fn convert_eng(input: &str) -> String {
    let mut out = input.to_string();
    let mut seen = Vec::new();
    for m in ENG_RE.find_iter(input) {
        let word = m.as_str();
        if seen.iter().any(|s: &&str| *s == word) {
            continue;
        }
        seen.push(word);
        if let Some(repl) = LEXICON.get(&word.to_lowercase().as_str()) {
            out = out.replace(word, repl);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_english_words() {
        assert_eq!(convert_eng("mp3 file game"), "mp3 파일 게임");
    }
}
