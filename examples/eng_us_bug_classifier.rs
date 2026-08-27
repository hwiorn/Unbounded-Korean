//! eng-us's analogue of eng_bug_classifier.rs. There's no separate human-verified
//! Hangul answer to check against here (eng-us doesn't try to match korean_go.tsv's
//! Korean loanword convention at all -- see eng_accuracy_check.rs's doc comment) --
//! the only meaningful "ground truth" is eng_us.dict's own trained cmudict-derived
//! phonemes. So this measures a single thing: for every word in eng_us.dict, does
//! the live decoder reproduce what p2g would render from that word's own trained
//! phonemes, or does it drift toward a different (if statistically more common)
//! pattern -- the exact failure mode that made "eng" wrong for "Rolland"/"coffee"
//! before the exact-dictionary-lookup fix.
//!
//! Run: cargo run --release --example eng_us_bug_classifier

use std::collections::HashMap;
use std::fs;

fn main() {
    let dict_text = fs::read_to_string("data/corpus/eng_us.dict").unwrap();
    let mut trained: HashMap<String, String> = HashMap::new();
    for line in dict_text.lines() {
        if let Some((word, phonemes)) = line.split_once('\t') {
            trained.insert(word.to_string(), phonemes.to_string());
        }
    }

    let mut correct = 0usize;
    let mut decoder_drift = Vec::new();
    let total = trained.len();

    for (word, phonemes) in &trained {
        let tokens: Vec<&str> = phonemes.split(' ').collect();
        let expected = korean_transliteration::p2g::phonemes_to_hangul(&tokens);
        let actual = match korean_transliteration::transliterate("eng-us", word) {
            Ok(h) => h,
            Err(_) => "<ERROR>".to_string(),
        };
        if actual == expected {
            correct += 1;
        } else {
            decoder_drift.push((word.clone(), expected, actual, phonemes.clone()));
        }
    }

    println!(
        "eng-us decoder fidelity to its own training data: {correct}/{total} = {:.2}%",
        100.0 * correct as f64 / total as f64
    );
    println!("decoder drift (trained phonemes render to X, live decoder gave Y): {}", decoder_drift.len());
    println!("\n--- samples (word, p2g(trained), live decode, trained_phonemes) ---");
    for (w, e, a, ph) in decoder_drift.iter().take(40) {
        println!("{w}: p2g(trained)={e} live={a} phonemes=[{ph}]");
    }

    let mut dump = String::new();
    for (w, e, a, ph) in &decoder_drift {
        dump.push_str(&format!("{w}\t{e}\t{a}\t{ph}\n"));
    }
    fs::write("target/eng_us_decoder_drift.tsv", dump).unwrap();
    println!("\nfull dump -> target/eng_us_decoder_drift.tsv");
}
