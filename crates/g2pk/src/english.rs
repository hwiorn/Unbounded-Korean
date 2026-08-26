use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

use crate::hangul::compose;

static ENG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z']+").unwrap());

/// word -> primary-pronunciation ARPABET tokens, parsed from the bundled CMUdict
/// (see resources/CMUDICT_LICENSE). Alternate pronunciations ("word(2)") and
/// non-alphabetic entries ("a.", "'bout") are skipped, leaving only the primary
/// entry per word -- matching upstream g2pK's `cmu[word][0]`.
static CMUDICT: Lazy<HashMap<&'static str, Vec<&'static str>>> = Lazy::new(|| {
    static RAW: &str = include_str!("resources/cmudict.dict");
    let mut map = HashMap::new();
    for line in RAW.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(word) = parts.next() else {
            continue;
        };
        if word.is_empty() || !word.chars().all(|c| c.is_ascii_lowercase() || c == '\'') {
            continue;
        }
        let phonemes: Vec<&str> = parts.collect();
        if phonemes.is_empty() {
            continue;
        }
        map.entry(word).or_insert(phonemes);
    }
    map
});

const SHORT_VOWELS: [&str; 7] = ["AE", "AH", "AX", "EH", "IH", "IX", "UH"];
const VOWELS: &str = "AEIOUY";
const CONSONANTS: &str = "BCDFGHJKLMNPQRSTVWXZ";
const SYLLABLE_FINAL_OR_CONSONANTS: &str = "$BCDFGHJKLMNPQRSTVWXZ";

/// Ported from g2pK's `utils.adjust`: strips ARPABET stress digits and folds a
/// handful of phoneme sequences into single pseudo-phonemes the table-driven
/// rules below key on directly.
fn adjust(arpabets: &[&str]) -> Vec<String> {
    let tokens: Vec<String> = arpabets
        .iter()
        .map(|s| s.chars().filter(|c| !c.is_ascii_digit()).collect::<String>())
        .collect();
    let mut s = format!(" {} $", tokens.join(" "));
    s = s.replace(" T S ", " TS ");
    s = s.replace(" D Z ", " DZ ");
    s = s.replace(" AW ER ", " AWER ");
    s = s.replace(" IH R $", " IH ER ");
    s = s.replace(" EH R $", " EH ER ");
    s = s.replace(" $", "");
    s.trim_matches(|c: char| c == '$' || c == ' ')
        .split_whitespace()
        .map(String::from)
        .collect()
}

fn to_choseong(p: &str) -> &'static str {
    match p {
        "B" | "V" => "\u{1107}",
        "CH" | "TS" => "\u{110E}",
        "D" | "DH" => "\u{1103}",
        "DZ" | "JH" | "Z" | "ZH" => "\u{110C}",
        "F" | "P" => "\u{1111}",
        "G" => "\u{1100}",
        "HH" => "\u{1112}",
        "K" => "\u{110F}",
        "L" | "R" => "\u{1105}",
        "M" => "\u{1106}",
        "N" => "\u{1102}",
        "NG" => "\u{110B}",
        "S" | "SH" | "TH" => "\u{1109}",
        "T" => "\u{1110}",
        "W" => "W",
        "Y" => "Y",
        _ => "",
    }
}

fn to_jungseong(p: &str) -> &'static str {
    match p {
        "AA" => "\u{1161}",
        "AE" => "\u{1162}",
        "AH" | "ER" => "\u{1165}",
        "AO" | "OW" => "\u{1169}",
        "AW" => "\u{1161}\u{110B}\u{116E}",
        "AWER" => "\u{1161}\u{110B}\u{116F}",
        "AY" => "\u{1161}\u{110B}\u{1175}",
        "EH" => "\u{1166}",
        "EY" => "\u{1166}\u{110B}\u{1175}",
        "IH" | "IY" => "\u{1175}",
        "OY" => "\u{1169}\u{110B}\u{1175}",
        "UH" | "UW" => "\u{116E}",
        _ => "",
    }
}

fn to_jongseong(p: &str) -> &'static str {
    match p {
        "B" | "P" | "V" => "\u{11B8}",
        "CH" => "\u{11BE}",
        "D" | "DH" => "\u{11AE}",
        "F" => "\u{11C1}",
        "G" | "K" => "\u{11A8}",
        "HH" => "\u{11C2}",
        "JH" | "Z" | "ZH" => "\u{11BD}",
        "L" | "R" => "\u{11AF}",
        "M" => "\u{11B7}",
        "N" => "\u{11AB}",
        "NG" | "W" | "Y" => "\u{11BC}",
        "S" | "SH" | "T" | "TH" => "\u{11BA}",
        _ => "",
    }
}

/// Ported from g2pK's `utils.reconstruct`: resolves the ASCII 'W'/'Y' glide
/// placeholders (and a couple of onset-cluster cleanups) left behind by the
/// per-phoneme loop into real Hangul vowel/lead jamo. Order matters -- later
/// pairs (the bare "Y"/"W" fallbacks) must run after every specific
/// combination has had a chance to match.
fn reconstruct(input: &str) -> String {
    const PAIRS: [(&str, &str); 23] = [
        ("\u{1100}\u{1173}W", "\u{1100}W"),
        ("\u{1112}\u{1173}W", "\u{1112}W"),
        ("\u{110F}\u{1173}W", "\u{110F}W"),
        ("\u{1102}Y\u{1165}", "\u{1102}\u{1175}\u{110B}\u{1165}"),
        ("\u{1103}Y\u{1165}", "\u{1103}\u{1175}\u{110B}\u{1165}"),
        ("\u{1105}Y\u{1165}", "\u{1105}\u{1175}\u{110B}\u{1165}"),
        ("Y\u{1175}", "\u{1175}"),
        ("Y\u{1161}", "\u{1163}"),
        ("Y\u{1162}", "\u{1164}"),
        ("Y\u{1165}", "\u{1167}"),
        ("Y\u{1166}", "\u{1168}"),
        ("Y\u{1169}", "\u{116D}"),
        ("Y\u{116E}", "\u{1172}"),
        ("W\u{1161}", "\u{116A}"),
        ("W\u{1162}", "\u{116B}"),
        ("W\u{1165}", "\u{116F}"),
        ("W\u{1169}", "\u{116F}"),
        ("W\u{116E}", "\u{116E}"),
        ("W\u{1166}", "\u{1170}"),
        ("W\u{1175}", "\u{1171}"),
        ("\u{1173}\u{1175}", "\u{1174}"),
        ("Y", "\u{1175}"),
        ("W", "\u{116E}"),
    ];
    let mut out = input.to_string();
    for (from, to) in PAIRS {
        out = out.replace(from, to);
    }
    out
}

/// True for a word with at least one letter, all of them uppercase (mirrors
/// Python's `str.isupper()`). g2pK skips these outright: an all-caps token is
/// more often an acronym or proper noun than the ordinary word CMUdict has an
/// entry for under the same spelling.
fn is_all_caps_word(word: &str) -> bool {
    let mut has_letter = false;
    for c in word.chars() {
        if c.is_alphabetic() {
            has_letter = true;
            if !c.is_uppercase() {
                return false;
            }
        }
    }
    has_letter
}

/// Converts English words embedded in `input` into Hangul, via CMUdict lookup
/// plus the official 외래어 표기법 (Foreign Loanword Orthography) phoneme rules,
/// faithfully ported from g2pK's `english.convert_eng`. Words not found in
/// CMUdict, and all-caps words, are left untouched, matching upstream behavior.
pub fn convert_eng(input: &str) -> String {
    let mut words: Vec<&str> = ENG_RE.find_iter(input).map(|m| m.as_str()).collect();
    words.sort_unstable();
    words.dedup();
    // Longest first, so replacing a short match (e.g. "car") can't corrupt an
    // already-substituted longer one (e.g. "card") still pending in `words`.
    words.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));

    let mut result = input.to_string();
    for eng_word in words {
        if is_all_caps_word(eng_word) {
            continue;
        }
        let word = eng_word.to_lowercase();
        let Some(arpabets) = CMUDICT.get(word.as_str()) else {
            continue;
        };
        let phonemes = adjust(arpabets);
        let mut ret = String::new();

        for i in 0..phonemes.len() {
            let p = phonemes[i].as_str();
            let p_prev = if i > 0 { phonemes[i - 1].as_str() } else { "^" };
            let p_next = if i + 1 < phonemes.len() {
                phonemes[i + 1].as_str()
            } else {
                "$"
            };
            // Mirrors g2pK's own `p_next2 = phonemes[i + 1] if i < len - 2 else "$"`
            // (reuses i+1, not i+2) -- ported as-is to match upstream behavior exactly.
            let p_next2 = if i + 2 < phonemes.len() {
                phonemes[i + 1].as_str()
            } else {
                "$"
            };

            let p_prev2 = &p_prev[..p_prev.len().min(2)];
            let p_next0 = p_next.chars().next().unwrap_or('$');
            let p_next20 = p_next2.chars().next().unwrap_or('$');
            let short_vowel_prev = SHORT_VOWELS.contains(&p_prev2);

            if matches!(p, "P" | "T" | "K") {
                // The first two arms both append the jongseong (word-end is a
                // subset of "not before L/R/M/N/vowel"), kept separate to
                // mirror 외래어 표기법 1항's own numbered rules 1 and 2.
                #[allow(clippy::if_same_then_else)]
                if short_vowel_prev && p_next == "$" {
                    ret.push_str(to_jongseong(p));
                } else if short_vowel_prev && !"AEIOULRMN".contains(p_next0) {
                    ret.push_str(to_jongseong(p));
                } else if "$BCDFGHJKLMNPQRSTVWXYZ".contains(p_next0) {
                    ret.push_str(to_choseong(p));
                    ret.push('\u{1173}');
                } else {
                    ret.push_str(to_choseong(p));
                }
            } else if matches!(p, "B" | "D" | "G") {
                ret.push_str(to_choseong(p));
                if SYLLABLE_FINAL_OR_CONSONANTS.contains(p_next0) {
                    ret.push('\u{1173}');
                }
            } else if matches!(p, "S" | "Z" | "F" | "V" | "TH" | "DH" | "SH" | "ZH") {
                ret.push_str(to_choseong(p));
                if matches!(p, "S" | "Z" | "F" | "V" | "TH" | "DH") {
                    if SYLLABLE_FINAL_OR_CONSONANTS.contains(p_next0) {
                        ret.push('\u{1173}');
                    }
                } else if p == "SH" {
                    if p_next0 == '$' {
                        ret.push('\u{1175}');
                    } else if CONSONANTS.contains(p_next0) {
                        ret.push('\u{1172}');
                    } else {
                        ret.push('Y');
                    }
                } else if p == "ZH" && SYLLABLE_FINAL_OR_CONSONANTS.contains(p_next0) {
                    ret.push('\u{1175}');
                }
            } else if matches!(p, "TS" | "DZ" | "CH" | "JH") {
                ret.push_str(to_choseong(p));
                if SYLLABLE_FINAL_OR_CONSONANTS.contains(p_next0) {
                    if matches!(p, "TS" | "DZ") {
                        ret.push('\u{1173}');
                    } else {
                        ret.push('\u{1175}');
                    }
                }
            } else if matches!(p, "M" | "N" | "NG") {
                if matches!(p, "M" | "N") && VOWELS.contains(p_next0) {
                    ret.push_str(to_choseong(p));
                } else {
                    ret.push_str(to_jongseong(p));
                }
            } else if p == "L" {
                if p_prev == "^" {
                    ret.push_str(to_choseong(p));
                } else if "$BCDFGHJKLPQRSTVWXZ".contains(p_next0) {
                    ret.push_str(to_jongseong(p));
                } else if matches!(p_prev, "M" | "N") {
                    ret.push_str(to_choseong(p));
                } else if VOWELS.contains(p_next0) {
                    ret.push('\u{11AF}');
                    ret.push('\u{1105}');
                } else if matches!(p_next, "M" | "N") && !VOWELS.contains(p_next20) {
                    ret.push('\u{11AF}');
                    ret.push('\u{1105}');
                    ret.push('\u{1173}');
                }
            } else if p == "ER" {
                let p_prev0 = p_prev.chars().next().unwrap_or('^');
                if VOWELS.contains(p_prev0) {
                    ret.push('\u{110B}');
                }
                ret.push_str(to_jungseong(p));
                if VOWELS.contains(p_next0) {
                    ret.push('\u{1105}');
                }
            } else if p == "R" {
                if VOWELS.contains(p_next0) {
                    ret.push_str(to_choseong(p));
                }
            } else if p.starts_with(|c: char| "AEIOU".contains(c)) {
                ret.push_str(to_jungseong(p));
            } else {
                ret.push_str(to_choseong(p));
            }
        }

        let ret = reconstruct(&ret);
        let ret = compose(&ret);
        let ret: String = ret
            .chars()
            .filter(|c| !('\u{1100}'..='\u{11FF}').contains(c))
            .collect();
        result = result.replace(eng_word, &ret);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_old_school_via_cmudict() {
        assert_eq!(
            convert_eng("\u{adf8} \u{c0ac}\u{b78c} \u{c880} old school\u{c774}\u{c57c}"),
            "\u{adf8} \u{c0ac}\u{b78c} \u{c880} \u{c62c}\u{b4dc} \u{c2a4}\u{cfe8}\u{c774}\u{c57c}"
        );
    }

    #[test]
    fn converts_file_and_game() {
        assert_eq!(convert_eng("mp3 file game"), "mp3 \u{d30c}\u{c77c} \u{ac8c}\u{c784}");
    }

    #[test]
    fn converts_bare_leading_vowel_word() {
        // "app" = AE1 P: the first phoneme is a vowel with no preceding
        // consonant jamo, exercising the silent-lead composition fix.
        assert_eq!(convert_eng("app"), "\u{c571}");
    }

    #[test]
    fn leaves_unknown_words_untouched() {
        assert_eq!(convert_eng("mp3"), "mp3");
    }

    #[test]
    fn converts_bliss_for_fyro_hotwords() {
        // handoff/2026-08-26-fyro-hotword-english-readings.md: a final /s/
        // needing its own syllable rather than a coda.
        assert_eq!(convert_eng("bliss"), "\u{be14}\u{b9ac}\u{c2a4}");
    }

    #[test]
    fn converts_mileage_using_cmudicts_primary_pronunciation() {
        // The handoff doc asks for "mileage" -> 마일리지, but CMUdict's
        // PRIMARY entry is "M AY1 L AH0 JH" (schwa) -- the same AH0 primary
        // choice it makes for every other unstressed "-age" word (cottage,
        // village, storage, package, message all list AH0 first too), and a
        // faithful cmu[word][0] port (what g2pK itself does) renders that
        // schwa as ᅥ, giving 마일러지, not the \u{c9c0}-ending
        // ᅵ-vowel form the doc asked for (which is CMUdict's *alternate*
        // "mileage(2)" pronunciation, "M AY1 L IH0 JH"). This is a limitation
        // of primary-only CMUdict lookup for this suffix, present in upstream
        // g2pK too, not a defect introduced by this port.
        assert_eq!(convert_eng("mileage"), "\u{b9c8}\u{c77c}\u{b7ec}\u{c9c0}");
    }

    #[test]
    fn leaves_all_caps_words_untouched() {
        // g2pK skips an all-caps token outright (more often an acronym or
        // proper noun than the ordinary word CMUdict has under that spelling).
        // fyro's own hotword_readings.rs lowercases before calling convert, so
        // this guard never blocks fyro -- it only protects other callers.
        assert_eq!(convert_eng("BLISS"), "BLISS");
    }
}
