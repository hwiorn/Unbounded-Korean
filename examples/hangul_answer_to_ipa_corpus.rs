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

// Matches korean_transliteration::p2g's recognized single-character phoneme set, plus
// 'ɯ' -- not a phoneme itself, but P2G's signal that the consonant right before it
// takes ㅡ as its own syllable nucleus (see p2g.rs's Unit::NeutralSyllable) rather than
// attaching to whatever vowel comes next (needed for e.g. "USA"'s middle S: 유에스에이,
// not 유에세이). Everything else (aspiration ʰ, tie-bars, palatalization ɕ/ʑ handled
// separately by tokenize_ipa) is dropped — P2G doesn't need it, and it would only
// teach the model to reproduce Korean-phonology noise instead of the English pattern.
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
            | 'ɯ'
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
    // Unicode-aware, not ASCII-only: German (Königen, Fräulein), and every other
    // Latin-script language beyond English, routinely need diacritics.
    !word.is_empty() && word.chars().all(|c| c.is_alphabetic() || c == '\'')
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
            // korean_phonemizer's medial_to_ipa follows traditional Korean IPA (ㅐ
            // is "ɛ", ㅔ is "e"), but korean-transliteration's P2G table follows the
            // English-IPA convention this corpus trains the model to decode
            // (CMUdict/misaki: "ɛ" is the vowel in "bed" -> ㅔ, "æ" is the vowel in
            // "cat" -> ㅐ). korean_phonemizer never emits "æ" itself, so remapping
            // its "ɛ" here is unambiguous -- without it, every Hangul-derived ㅐ
            // (e.g. "bank" 뱅크) trained the model to round-trip back as ㅔ instead.
            tokens.push(if c == 'ɛ' { "æ".to_string() } else { c.to_string() });
        }
        rest = chars.as_str();
    }
    tokens
}

/// Korean's plain ㅂ/ㄷ/ㄱ are phonetically realized as voiceless [p]/[t]/[k]
/// word-initially (a real, correctly-modeled fact about Korean phonetics --
/// korean_phonemizer's `is_voicing_context` only voices a lenis stop after a vowel or
/// sonorant coda, which a word's first syllable never has). That's the right acoustic
/// answer for Korean pronunciation, but it destroys the ㅂ-vs-ㅍ/ㄷ-vs-ㅌ/ㄱ-vs-ㅋ
/// distinction this derivation needs to recover which English letter a word-initial
/// Hangul syllable stands for ("BBC" -> 비비시 was training on "p i b i s i", losing B
/// entirely). The Hangul spelling itself is unambiguous -- ㅂ always means B, never P
/// -- so this corrects just the first token using the orthographic lead consonant.
fn word_initial_lenis_voicing_correction(hangul: &str) -> Option<(&'static str, &'static str)> {
    let (lead, _, _) = g2pk::hangul::decompose_char(hangul.chars().next()?)?;
    match lead {
        'ᄇ' => Some(("p", "b")),
        'ᄃ' => Some(("t", "d")),
        'ᄀ' => Some(("k", "ɡ")),
        _ => None,
    }
}

/// Returns (filtered-for-Phonetisaurus-training, raw-unfiltered-korean-phonemizer-ipa).
/// The raw form is preserved as its own artifact (not discarded) even though only the
/// filtered form feeds this crate's training corpus — the full Korean phonetic detail
/// (palatalization, aspiration, coda filler vowels) that gets dropped here is exactly
/// the kind of data a future Korean TTS/ASR model would want, so it's kept rather than
/// thrown away.
fn hangul_to_ipa(hangul: &str) -> Option<(String, String)> {
    let phonemized = korean_phonemizer::phonemize_ko(hangul).ok()?;
    let mut filtered = tokenize_ipa(&phonemized.ipa);
    if let Some((voiceless, voiced)) = word_initial_lenis_voicing_correction(hangul) {
        if let Some(first) = filtered.first_mut() {
            if first == voiceless {
                *first = voiced.to_string();
            }
        }
    }
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
    fn drops_unrecognized_characters_but_keeps_the_neutral_syllable_marker() {
        // Aspiration/tense marks carry no P2G target and are dropped, but ɯ (the
        // neutral-syllable marker P2G's Unit::NeutralSyllable consumes) is kept.
        assert_eq!(tokenize_ipa("tʰɯs͈ɯ"), vec!["t", "ɯ", "s", "ɯ"]);
    }

    #[test]
    fn corrects_word_initial_lenis_stop_voicing_for_bbc() {
        // "비비시" phonemizes with a devoiced word-initial ㅂ ([p], correct Korean
        // acoustics), which would otherwise train "BBC" on a lost B.
        let (filtered, raw) = hangul_to_ipa("비비시").unwrap();
        assert_eq!(filtered, "b i b i s i");
        assert!(raw.starts_with('p'), "raw acoustic form must stay untouched: {raw:?}");
    }

    #[test]
    fn does_not_correct_a_genuinely_aspirated_word_initial_consonant() {
        // "커피" (coffee) starts with ㅋ (aspirated K), which already renders as "k" --
        // there is no voiceless/voiced pair to correct here.
        let (filtered, _) = hangul_to_ipa("커피").unwrap();
        assert_eq!(filtered, "k ʌ p i");
    }

    #[test]
    fn remaps_korean_phonemizers_ae_symbol_to_match_p2gs_english_ipa_convention() {
        // korean_phonemizer's medial_to_ipa follows traditional Korean IPA, where ㅐ
        // is "ɛ" and ㅔ is "e" -- but korean-transliteration's P2G table follows the
        // English-IPA convention this corpus is trained to decode (CMUdict/misaki:
        // "ɛ" is the vowel in "bed", mapping to ㅔ; "æ" is the vowel in "cat",
        // mapping to ㅐ). Left unmapped, every Hangul-answer-derived ㅐ in the
        // corpus (e.g. "bank" 뱅크) silently trained the model to round-trip it back
        // as ㅔ instead.
        let (filtered, _) = hangul_to_ipa("뱅크").unwrap();
        assert_eq!(filtered, "b æ ŋ k ɯ");
    }

    #[test]
    fn accepts_words_with_non_ascii_letters_but_rejects_multi_word_phrases() {
        assert!(is_clean_word("Königen"));
        assert!(is_clean_word("Fräulein"));
        assert!(!is_clean_word("Guido van Rossum"));
        assert!(!is_clean_word(""));
    }
}
