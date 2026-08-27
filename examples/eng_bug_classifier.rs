//! For every korean_go.tsv mismatch, distinguishes a P2G rendering bug (the exact
//! trained phonemes for this word still don't render to the expected Hangul) from a
//! model/decoding issue (the trained phonemes DO render correctly via P2G, but the
//! live decoder chose different phonemes than what was trained) from a pure
//! out-of-vocabulary generalization gap (the word isn't in eng.dict at all).
//!
//! Run: cargo run --release --example eng_bug_classifier

use std::collections::HashMap;
use std::fs;

/// Some words appear more than once in korean_go.tsv with genuinely different
/// Hangul answers (329 of them, e.g. "Daniel" -> 대니얼 eighteen times, 다니엘
/// once). Checking against whichever occurrence happens to load last into a
/// HashMap is arbitrary -- it misclassified 274 of 286 "p2g_bug" cases this way
/// (197 genuine ties, 58 where eng.dict trained on a minority answer instead of
/// the clear majority). This picks the most-frequent answer per word instead,
/// matching hangul_answer_to_ipa_corpus.rs's own majority_hangul so this checks
/// against the same answer eng.dict was actually trained on.
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

    let dict_text = fs::read_to_string("data/corpus/eng.dict").unwrap();
    let mut trained: HashMap<String, String> = HashMap::new();
    for line in dict_text.lines() {
        if let Some((word, phonemes)) = line.split_once('\t') {
            trained.insert(word.to_string(), phonemes.to_string());
        }
    }

    let mut correct = 0;
    let mut p2g_bug = Vec::new();
    let mut decoder_issue = Vec::new();
    let mut oov = Vec::new();

    for (word, expected) in &truth {
        let actual = match korean_transliteration::transliterate("eng", word) {
            Ok(h) => h,
            Err(_) => "<ERROR>".to_string(),
        };
        if &actual == expected {
            correct += 1;
            continue;
        }
        match trained.get(word).or_else(|| trained.get(&word.to_lowercase())) {
            Some(phonemes) => {
                let tokens: Vec<&str> = phonemes.split(' ').collect();
                let from_training = korean_transliteration::p2g::phonemes_to_hangul(&tokens);
                if &from_training == expected {
                    decoder_issue.push((word.clone(), expected.clone(), actual, phonemes.clone()));
                } else {
                    p2g_bug.push((
                        word.clone(),
                        expected.clone(),
                        actual,
                        phonemes.clone(),
                        from_training,
                    ));
                }
            }
            None => oov.push((word.clone(), expected.clone(), actual)),
        }
    }

    let total = truth.len();
    println!("correct: {correct}/{total} = {:.1}%", 100.0 * correct as f64 / total as f64);
    println!("mismatches: {}", total - correct);
    println!("  p2g_bug (trained phonemes exist, P2G still renders wrong): {}", p2g_bug.len());
    println!("  decoder_issue (trained phonemes render right, but decoder didn't pick them): {}", decoder_issue.len());
    println!("  oov (word not in eng.dict at all): {}", oov.len());

    println!("\n--- p2g_bug samples (word, expected, actual, trained_phonemes, p2g(trained_phonemes)) ---");
    for (w, e, a, ph, from_t) in p2g_bug.iter().take(60) {
        println!("{w}: expected={e} actual={a} trained_phonemes=[{ph}] p2g(trained)={from_t}");
    }

    println!("\n--- decoder_issue samples (word, expected, actual, trained_phonemes) ---");
    for (w, e, a, ph) in decoder_issue.iter().take(30) {
        println!("{w}: expected={e} actual={a} trained_phonemes=[{ph}]");
    }

    let dump = |path: &str, rows: &[(String, String, String)]| {
        let mut s = String::new();
        for (w, e, a) in rows {
            s.push_str(&format!("{w}\t{e}\t{a}\n"));
        }
        fs::write(path, s).unwrap();
    };
    dump("target/eng_oov_mismatches.tsv", &oov);

    let mut p2g_dump = String::new();
    for (w, e, a, ph, from_t) in &p2g_bug {
        p2g_dump.push_str(&format!("{w}\t{e}\t{a}\t{ph}\t{from_t}\n"));
    }
    fs::write("target/eng_p2g_bug_mismatches.tsv", p2g_dump).unwrap();

    let mut decoder_dump = String::new();
    for (w, e, a, ph) in &decoder_issue {
        decoder_dump.push_str(&format!("{w}\t{e}\t{a}\t{ph}\n"));
    }
    fs::write("target/eng_decoder_issue_mismatches.tsv", decoder_dump).unwrap();
    println!("\nfull dumps written to target/eng_{{oov,p2g_bug,decoder_issue}}_mismatches.tsv");
}
