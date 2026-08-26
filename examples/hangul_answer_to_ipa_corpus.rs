// Converts a (word, Hangul-answer) TSV — e.g. data/corpus/hsl_seed.tsv (this
// session's eng.hsl fix cases: SKT, NAVER, KT, AI) or data/corpus/korean_go.tsv
// (muik/transliteration's 31,898 government-sourced English->Korean pairs) — into
// (word, simplified-IPA-phonemes) training pairs, by running the Hangul answer
// through korean_phonemizer's real Korean G2P and keeping only the characters that
// are recognized English phonemes in korean_transliteration's P2G table.
//
// This reuses the REAL, human-verified answer instead of guessing English
// pronunciation (misaki) or hand-authoring phoneme tables (both error-prone, as this
// session found) — the G2P(Phonetisaurus) -> IPA -> P2G pipeline stays unchanged;
// this only supplies better training data for it, matching the design requirement
// that other source languages could reuse the identical technique later since
// korean_phonemizer's role here is Hangul -> phonemes, not English-specific.
//
// Usage: hangul_answer_to_ipa_corpus <input.tsv> <output.tsv> [--word-col N] [--hangul-col N]
//
// Input format: tab-separated, at least two columns; by default column 0 is the
// word and the LAST column is the Hangul answer (matches both hsl_seed.tsv's
// lang\tword\thangul and korean_go.tsv's word\thangul).

use std::collections::HashSet;
use std::fs;
use std::io::Write;

// Matches korean_transliteration::p2g's recognized single-character phoneme set.
// Everything else (Korean-phonology-specific artifacts: aspiration ʰ, tie-bars,
// palatalization ɕ/ʑ, the neutral-vowel filler ɯ that Korean codas always carry but
// English doesn't) is dropped — P2G doesn't need it, and it would only teach the
// model to reproduce Korean-phonology noise instead of the English pattern.
fn is_recognized_phoneme_char(c: char) -> bool {
    matches!(
        c,
        'æ' | 'ɛ'
            | 'ə'
            | 'e'
            | 'ᵻ'
            | 'ɜ'
            | 'ʌ'
            | 'ɔ'
            | 'ɚ'
            | 'ɝ'
            | 'ɑ'
            | 'a'
            | 'i'
            | 'ɪ'
            | 'u'
            | 'ʊ'
            | 'o'
            | 'ɡ'
            | 'g'
            | 'k'
            | 't'
            | 'd'
            | 'p'
            | 'b'
            | 'f'
            | 'v'
            | 's'
            | 'θ'
            | 'z'
            | 'ð'
            | 'ʃ'
            | 'ʒ'
            | 'h'
            | 'm'
            | 'n'
            | 'ŋ'
            | 'l'
            | 'ɫ'
            | 'r'
            | 'ɹ'
            | 'w'
            | 'j'
    )
}

fn is_clean_word(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_alphabetic() || c == '\'')
}

/// Korean ㅈ/ㅊ/ㅉ are alveolo-palatal affricates in korean_phonemizer's IPA output --
/// a tie-barred sequence built from a stop plus U+0255 (ɕ, the alveolo-palatal
/// fricative), not the plain postalveolar ʃ/ʒ this module's single-character filter
/// already recognizes. Left to plain per-character filtering, ɕ (and the combining
/// tie bar/tense mark) are silently dropped, leaving only the bare stop -- so ㅈ/ㅊ
/// are mis-tokenized as plain T/D and corrupt any word containing this extremely
/// common Korean consonant pair (measured at ~20% of korean_go/muik entries). Ordered
/// longest-pattern-first: "t\u{361}\u{255}" is a prefix of "t\u{361}\u{255}\u{2b0}", so the
/// aspirated form must match before the plain one.
const AFFRICATE_PATTERNS: &[(&str, &str)] = &[
    ("t\u{348}\u{361}\u{255}", "dʒ"), // ㅉ (tense) -- P2G has no tense-consonant target
    ("t\u{361}\u{255}\u{2b0}", "tʃ"), // ㅊ (aspirated)
    ("t\u{361}\u{255}", "dʒ"),        // ㅈ (plain, voiceless allophone)
    ("d\u{361}\u{255}", "dʒ"),        // ㅈ (plain, voiced intervocalic allophone)
];

/// Tokenizes `ipa` into the phoneme units `korean-transliteration`'s P2G table
/// recognizes: an affricate pattern becomes one two-character token ("dʒ"/"tʃ"), and
/// every other recognized character becomes its own one-character token. Anything not
/// recognized (aspiration, tense marks, the coda filler vowel ɯ, tie bars) is dropped.
fn tokenize_ipa(ipa: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = ipa;
    'outer: while !rest.is_empty() {
        for (pattern, token) in AFFRICATE_PATTERNS {
            if let Some(stripped) = rest.strip_prefix(pattern) {
                tokens.push(token.to_string());
                rest = stripped;
                continue 'outer;
            }
        }
        let mut chars = rest.chars();
        let c = chars.next().expect("rest is non-empty");
        if is_recognized_phoneme_char(c) {
            tokens.push(c.to_string());
        }
        rest = chars.as_str();
    }
    tokens
}

/// Returns (filtered-for-Phonetisaurus-training, raw-unfiltered-korean-phonemizer-ipa).
/// The raw form is preserved as its own artifact (not discarded) even though only the
/// filtered form feeds this crate's training corpus — the full Korean phonetic detail
/// (palatalization, aspiration, coda filler vowels) that gets dropped here is exactly
/// the kind of data a future Korean TTS/ASR model would want, so it's kept rather than
/// thrown away.
fn hangul_to_ipa(hangul: &str) -> Option<(String, String)> {
    let phonemized = korean_phonemizer::phonemize_ko(hangul).ok()?;
    let filtered = tokenize_ipa(&phonemized.ipa);
    if filtered.is_empty() {
        None
    } else {
        Some((filtered.join(" "), phonemized.ipa))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input_path = args
        .get(1)
        .expect("usage: <input.tsv> <filtered-output.tsv> <raw-output.tsv>");
    let filtered_path = args
        .get(2)
        .expect("usage: <input.tsv> <filtered-output.tsv> <raw-output.tsv>");
    let raw_path = args
        .get(3)
        .expect("usage: <input.tsv> <filtered-output.tsv> <raw-output.tsv>");

    let content = fs::read_to_string(input_path).expect("read input");
    let mut filtered_out = fs::File::create(filtered_path).expect("create filtered output");
    let mut raw_out = fs::File::create(raw_path).expect("create raw output");

    let mut seen: HashSet<String> = HashSet::new();
    let mut written = 0usize;
    let mut skipped = 0usize;
    for line in content.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            skipped += 1;
            continue;
        }
        let word = cols[0];
        let hangul = cols[cols.len() - 1];
        if !is_clean_word(word) || !seen.insert(word.to_string()) {
            skipped += 1;
            continue;
        }
        match hangul_to_ipa(hangul) {
            Some((filtered, raw)) => {
                writeln!(filtered_out, "{word}\t{filtered}").expect("write filtered");
                writeln!(raw_out, "{word}\t{hangul}\t{raw}").expect("write raw");
                written += 1;
            }
            None => {
                eprintln!("skip (no usable phonemes): {word} ({hangul})");
                skipped += 1;
            }
        }
    }
    eprintln!("done: {written} written, {skipped} skipped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_plain_affricate_as_dz() {
        // "가지" (garbage's second syllable) -> k a d\u{361}\u{255}i; the affricate must
        // survive as one "dʒ" token, not collapse to a bare "d".
        assert_eq!(
            tokenize_ipa("kad\u{361}\u{255}i"),
            vec!["k", "a", "dʒ", "i"]
        );
    }

    #[test]
    fn tokenizes_aspirated_affricate_as_tsh_before_plain_prefix_matches() {
        // "지피티" (GPT) -> t\u{361}\u{255}ipʰit\u{361}\u{255}\u{2b0}i; the aspirated form
        // must win over its own unaspirated prefix.
        assert_eq!(
            tokenize_ipa("t\u{361}\u{255}ipʰit\u{361}\u{255}\u{2b0}i"),
            vec!["dʒ", "i", "p", "i", "tʃ", "i"]
        );
    }

    #[test]
    fn tokenizes_tense_affricate_as_dz() {
        assert_eq!(
            tokenize_ipa("at\u{348}\u{361}\u{255}a"),
            vec!["a", "dʒ", "a"]
        );
    }

    #[test]
    fn drops_unrecognized_characters() {
        // Aspiration/tense marks and the coda filler vowel ɯ carry no P2G target.
        assert_eq!(tokenize_ipa("tʰɯs͈ɯ"), vec!["t", "s"]);
    }
}
