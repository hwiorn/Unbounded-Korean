//! Measures a trained English model's exact-match rate against
//! data/corpus/korean_go.tsv (국립국어원 official loanword spellings) -- a much
//! larger and more independent benchmark than tests/korean_transliteration_cases.rs's
//! hand-picked cases, since it covers every single-word entry in that source
//! regardless of whether it happens to be memorized correctly.
//!
//! korean_go.tsv is Korean loanword CONVENTION, not raw American pronunciation --
//! this is the right benchmark for "eng" (trained on that same convention) but only
//! a rough, expected-to-be-low sanity check for "eng-us" (trained on cmudict's
//! actual pronunciation, which legitimately disagrees with convention often, see
//! "mileage" 마일리지 vs 마일레즈).
//!
//! Run: cargo run --release --example eng_accuracy_check [eng|eng-us]

use std::collections::HashMap;
use std::env;
use std::fs;

/// Some words appear more than once in korean_go.tsv with genuinely different
/// Hangul answers (329 of them, e.g. "Daniel" -> 대니얼 eighteen times, 다니엘
/// once) -- checking against whichever occurrence happens to load last into a
/// HashMap is arbitrary and can grade a correct majority-convention answer as
/// wrong. This picks the most-frequent answer per word instead (ties broken by
/// first occurrence), matching hangul_answer_to_ipa_corpus.rs's own
/// majority_hangul so the benchmark checks against the same answer eng.dict was
/// actually trained on.
fn majority_hangul(answers: &[String]) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for a in answers {
        *counts.entry(a.as_str()).or_insert(0) += 1;
    }
    let max = *counts.values().max().expect("answers is non-empty");
    answers
        .iter()
        .find(|a| counts[a.as_str()] == max)
        .expect("some answer reaches the max count")
        .clone()
}

fn main() {
    let lang = env::args().nth(1).unwrap_or_else(|| "eng".to_string());
    let korean_go = fs::read_to_string("data/corpus/korean_go.tsv").unwrap();
    let mut order: Vec<String> = Vec::new();
    let mut answers: HashMap<String, Vec<String>> = HashMap::new();
    for line in korean_go.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 2 {
            continue;
        }
        let (word, hangul) = (cols[0], cols[1]);
        if word.chars().all(|c| c.is_ascii_alphabetic()) {
            answers
                .entry(word.to_string())
                .or_insert_with(|| {
                    order.push(word.to_string());
                    Vec::new()
                })
                .push(hangul.to_string());
        }
    }
    let truth: HashMap<String, String> = order
        .into_iter()
        .map(|word| {
            let hangul = majority_hangul(&answers[&word]);
            (word, hangul)
        })
        .collect();
    let total = truth.len();
    let mut correct = 0;
    let mut all_wrong = Vec::new();
    for (word, expected) in &truth {
        match korean_transliteration::transliterate(&lang, word) {
            Ok(actual) if &actual == expected => correct += 1,
            Ok(actual) => all_wrong.push((word.clone(), expected.clone(), actual)),
            Err(_) => all_wrong.push((word.clone(), expected.clone(), "<ERROR>".to_string())),
        }
    }
    println!("{lang} vs korean_go.tsv (single-word entries): {correct}/{total} = {:.1}%", 100.0 * correct as f64 / total as f64);
    println!("--- sample mismatches ---");
    for (word, expected, actual) in all_wrong.iter().take(40) {
        println!("{word}: expected {expected}, got {actual}");
    }
    let dump_path = format!("target/{}_mismatches.tsv", lang.replace('-', "_"));
    let mut dump = String::new();
    for (word, expected, actual) in &all_wrong {
        dump.push_str(&format!("{word}\t{expected}\t{actual}\n"));
    }
    fs::write(&dump_path, dump).unwrap();
    println!("--- full mismatch dump: {} rows -> {dump_path} ---", all_wrong.len());
}
