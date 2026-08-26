//! Measures the trained eng.fst.gz model's exact-match rate against
//! data/corpus/korean_go.tsv (국립국어원 official loanword spellings) -- a much
//! larger and more independent benchmark than tests/korean_transliteration_cases.rs's
//! hand-picked cases, since it covers every single-word entry in that source
//! regardless of whether it happens to be memorized correctly.
//!
//! Run: cargo run --release --example eng_accuracy_check

use std::collections::HashMap;
use std::fs;

fn main() {
    let korean_go = fs::read_to_string("data/corpus/korean_go.tsv").unwrap();
    let mut truth: HashMap<String, String> = HashMap::new();
    for line in korean_go.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 2 {
            continue;
        }
        let (word, hangul) = (cols[0], cols[1]);
        if word.chars().all(|c| c.is_ascii_alphabetic()) {
            truth.insert(word.to_string(), hangul.to_string());
        }
    }
    let total = truth.len();
    let mut correct = 0;
    let mut sample_wrong = Vec::new();
    for (word, expected) in &truth {
        match korean_transliteration::transliterate("eng", word) {
            Ok(actual) if &actual == expected => correct += 1,
            Ok(actual) => {
                if sample_wrong.len() < 40 {
                    sample_wrong.push(format!("{word}: expected {expected}, got {actual}"));
                }
            }
            Err(_) => {
                if sample_wrong.len() < 40 {
                    sample_wrong.push(format!("{word}: ERROR (no path)"));
                }
            }
        }
    }
    println!("korean_go.tsv (single-word entries): {correct}/{total} = {:.1}%", 100.0 * correct as f64 / total as f64);
    println!("--- sample mismatches ---");
    for s in &sample_wrong {
        println!("{s}");
    }
}
