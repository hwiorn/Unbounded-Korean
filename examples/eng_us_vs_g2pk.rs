//! Cross-checks korean-transliteration's "eng-us" (cmudict-trained Phonetisaurus + P2G)
//! against crates/g2pk's independent, self-contained cmudict-based pipeline
//! (its own ARPABET rules + composition logic, no shared code at all). Neither
//! is "ground truth" -- g2pk documents its own AH0/schwa limitation -- but a
//! disagreement between two independently-built cmudict pipelines is worth a
//! look: it's either a genuine P2G bug in one of them, or an inherent
//! schwa-rendering ambiguity (see p2g.rs's schwa comment), and this sorts the
//! two apart by whether the *structure* (syllable count, consonants) matches,
//! not just individual vowel jamo choices.
//!
//! Run: cargo run --release --example eng_us_vs_g2pk

use std::fs;

/// Coarse structural fingerprint: drop vowel jamo, keep lead/tail consonants
/// and syllable count, so a pure vowel-choice difference (schwa -> ㅓ vs ㅔ
/// vs ㅏ) doesn't count as a "structural" mismatch.
fn structural_fingerprint(hangul: &str) -> String {
    let mut out = String::new();
    for ch in hangul.chars() {
        let code = ch as u32;
        if !(0xAC00..=0xD7A3).contains(&code) {
            out.push(ch);
            continue;
        }
        let idx = code - 0xAC00;
        let lead = idx / (21 * 28);
        let tail = idx % 28;
        out.push(char::from_u32(0x1100 + lead).unwrap_or('?'));
        if tail != 0 {
            out.push(char::from_u32(0x11A7 + tail).unwrap_or('?'));
        }
    }
    out
}

fn main() {
    let g2p = g2pk::G2p::new().expect("g2pk::G2p::new");
    let cmudict_text = fs::read_to_string("data/corpus/eng_us.dict").unwrap();
    let mut words: Vec<&str> = cmudict_text
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(w, _)| w)
        .filter(|w| w.chars().all(|c| c.is_ascii_lowercase() || c == '\''))
        .collect();
    words.sort_unstable();

    let mut same = 0usize;
    let mut vowel_only_diff = Vec::new();
    let mut structural_diff = Vec::new();
    let mut g2pk_error = 0usize;

    for word in &words {
        let ours = match korean_transliteration::transliterate("eng-us", word) {
            Ok(h) => h,
            Err(_) => continue,
        };
        let theirs = g2p.convert(word).unwrap_or_default();
        if theirs.is_empty() || theirs == *word {
            g2pk_error += 1;
            continue;
        }
        if ours == theirs {
            same += 1;
            continue;
        }
        if structural_fingerprint(&ours) == structural_fingerprint(&theirs) {
            vowel_only_diff.push((word.to_string(), ours, theirs));
        } else {
            structural_diff.push((word.to_string(), ours, theirs));
        }
    }

    println!("total words compared: {}", words.len());
    println!("identical: {same}");
    println!(
        "vowel-jamo-only difference (likely schwa/vowel-choice ambiguity, not necessarily a bug): {}",
        vowel_only_diff.len()
    );
    println!(
        "structural difference (different consonants/syllable count -- worth inspecting): {}",
        structural_diff.len()
    );
    println!("g2pk gave no real answer (not in its cmudict copy): {g2pk_error}");

    let mut dump = String::new();
    for (w, ours, theirs) in &structural_diff {
        dump.push_str(&format!("{w}\t{ours}\t{theirs}\n"));
    }
    fs::write("target/eng_us_vs_g2pk_structural.tsv", dump).unwrap();
    println!("\nfull structural-diff dump -> target/eng_us_vs_g2pk_structural.tsv");

    println!("\n--- structural diff samples (word, ours(eng-us), g2pk) ---");
    for (w, ours, theirs) in structural_diff.iter().take(60) {
        println!("{w}: ours={ours} g2pk={theirs}");
    }
}
