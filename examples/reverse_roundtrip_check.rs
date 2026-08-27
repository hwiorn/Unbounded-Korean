//! Validates `korean_transliteration::reverse::hangul_to_phonemes` at scale: for
//! every single-word entry in the given TSV files (word<TAB>hangul), checks that
//! `p2g::phonemes_to_hangul(&hangul_to_phonemes(hangul).unwrap()) == hangul`.
//! Reports the failure/unsupported breakdown so remaining gaps can be triaged
//! before this replaces korean_phonemizer in the corpus-generation pipeline.
//!
//! Run: cargo run --release --example reverse_roundtrip_check <tsv...>

use korean_transliteration::{p2g, reverse};
use std::fs;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let paths = if paths.is_empty() {
        vec!["data/corpus/korean_go.tsv".to_string()]
    } else {
        paths
    };

    let mut total = 0usize;
    let mut ok = 0usize;
    let mut unsupported = 0usize;
    let mut mismatched = Vec::new();

    for path in &paths {
        let content = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        for line in content.lines() {
            let Some((word, hangul)) = line.split_once('\t') else {
                continue;
            };
            if word.is_empty() || !word.chars().all(|c| c.is_alphabetic() || c == '\'') {
                continue;
            }
            total += 1;
            match reverse::hangul_to_phonemes(hangul) {
                None => unsupported += 1,
                Some(tokens) => {
                    let back = p2g::phonemes_to_hangul(&tokens);
                    if back == hangul {
                        ok += 1;
                    } else {
                        mismatched.push((word.to_string(), hangul.to_string(), tokens, back));
                    }
                }
            }
        }
    }

    println!("total single-word entries checked: {total}");
    println!(
        "round-trips exactly: {ok} ({:.1}%)",
        100.0 * ok as f64 / total as f64
    );
    println!(
        "unsupported (no literal encoding -- word skipped, not trained wrong): {unsupported} ({:.1}%)",
        100.0 * unsupported as f64 / total as f64
    );
    println!(
        "round-trip MISMATCH (genuine bug -- reverse produced tokens but forward disagrees): {}",
        mismatched.len()
    );
    for (word, hangul, tokens, back) in mismatched.iter().take(60) {
        println!("  {word}: hangul={hangul} tokens={tokens:?} p2g(tokens)={back}");
    }
}
