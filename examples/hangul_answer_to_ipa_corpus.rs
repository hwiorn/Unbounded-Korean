// Converts a (word, Hangul-answer) TSV — e.g. data/corpus/hsl_seed.tsv (this
// session's eng.hsl fix cases: SKT, NAVER, KT, AI) or data/corpus/korean_go.tsv
// (muik/transliteration's 31,898 government-sourced English->Korean pairs) — into
// (word, phonemes) training pairs, by running the Hangul answer through
// korean_transliteration::reverse's literal, context-free inverse of p2g's own
// tables and verifying it round-trips exactly back through p2g::phonemes_to_hangul
// before accepting it.
//
// This used to go through korean_phonemizer's real Korean G2P instead. That's the
// wrong tool for this job: it applies genuine Korean phonology (lateralization,
// palatalization, tensification, position-dependent voicing) that changes what the
// WRITTEN answer says to match how a Korean speaker would actually pronounce it
// ("월넛" -> "월럳" via ㄹ+ㄴ lateralization) -- which p2g's forward direction, having
// no Korean phonology of its own, can never undo. reverse::hangul_to_phonemes is a
// genuine mathematical inverse of p2g's own tables instead, so it can't drift out of
// sync with them the way a separately-maintained real-pronunciation engine can.
//
// Usage: hangul_answer_to_ipa_corpus <input.tsv> <filtered-output.tsv> <raw-output.tsv>
//
// Input format: tab-separated, at least two columns; column 0 is the word and the
// LAST column is the Hangul answer (matches both hsl_seed.tsv's lang\tword\thangul
// and korean_go.tsv's word\thangul).

use std::collections::HashSet;
use std::fs;
use std::io::Write;

fn is_clean_word(word: &str) -> bool {
    // Unicode-aware, not ASCII-only: German (Königen, Fräulein), and every other
    // Latin-script language beyond English, routinely need diacritics.
    !word.is_empty() && word.chars().all(|c| c.is_alphabetic() || c == '\'')
}

/// Returns (space-joined phonemes that verifiably round-trip back to `hangul` via
/// p2g, real-pronunciation IPA kept only as a byproduct for a future Korean
/// TTS/ASR use -- not fed into this crate's training corpus). `None` if `hangul`
/// uses a syllable shape p2g's forward table can never produce at all, or the
/// reverse tokens don't actually round-trip (the already-known W-glide
/// onset-vs-coda ambiguity, or a written batchim immediately before a null-onset
/// vowel syllable, both irreducible in p2g's current phoneme alphabet -- see
/// reverse.rs's module doc) -- either way, writing that pair would teach the model
/// to reproduce something it structurally cannot, so it's skipped rather than
/// trained wrong.
fn hangul_to_ipa(hangul: &str) -> Option<(String, String)> {
    let tokens = korean_transliteration::reverse::hangul_to_phonemes(hangul)?;
    if korean_transliteration::p2g::phonemes_to_hangul(&tokens) != hangul {
        return None;
    }
    let raw = korean_phonemizer::phonemize_ko(hangul)
        .map(|p| p.ipa)
        .unwrap_or_default();
    Some((tokens.join(" "), raw))
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
                eprintln!("skip (no round-tripping phoneme encoding): {word} ({hangul})");
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
    fn accepts_words_with_non_ascii_letters_but_rejects_multi_word_phrases() {
        assert!(is_clean_word("Königen"));
        assert!(is_clean_word("Fräulein"));
        assert!(!is_clean_word("Guido van Rossum"));
        assert!(!is_clean_word(""));
    }

    #[test]
    fn converts_a_real_answer_into_its_round_tripping_phonemes() {
        let (filtered, _) = hangul_to_ipa("커피").unwrap();
        assert_eq!(filtered, "k ʌ p i");
    }

    #[test]
    fn does_not_lateralize_walnut_the_way_the_old_real_pronunciation_pipeline_did() {
        // korean_phonemizer's phonemize_ko("월넛") used to return "월럳" (Korean's
        // own ㄹ+ㄴ lateralization, applied by g2pk::G2p::convert() before
        // korean_phonemizer's own code even runs) -- p2g had no way to undo that,
        // so training on it could never reproduce "월넛" again. The literal
        // reverse has no such phonology to apply in the first place.
        let (filtered, _) = hangul_to_ipa("월넛").unwrap();
        let tokens: Vec<String> = filtered.split(' ').map(String::from).collect();
        assert_eq!(korean_transliteration::p2g::phonemes_to_hangul(&tokens), "월넛");
    }

    #[test]
    fn skips_an_unrepresentable_batchim_instead_of_training_a_lossy_guess() {
        // 힣 (ㅎ+ㅣ+ㅎ): a ㅎ batchim is not one of the seven batchim jamo p2g's
        // own as_tail can ever output, so no phoneme sequence could round-trip
        // through it either way -- must be skipped, not silently guessed.
        assert!(hangul_to_ipa("힣").is_none());
    }
}
