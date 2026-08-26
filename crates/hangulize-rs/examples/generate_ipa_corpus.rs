// Bulk-generates (word, simplified IPA) training pairs for the korean-transliteration
// Phonetisaurus corpus by running every word in a wordlist through the existing
// misaki-based English G2P. See docs/specs/2026-08-26-korean-transliteration-design.md.
//
// Usage: generate_ipa_corpus <wordlist-path> <output-tsv-path>
use std::fs;
use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let wordlist_path = args.get(1).expect("usage: generate_ipa_corpus <wordlist> <out.tsv>");
    let out_path = args.get(2).expect("usage: generate_ipa_corpus <wordlist> <out.tsv>");

    let words = fs::File::open(wordlist_path).expect("wordlist must exist");
    let mut out = fs::File::create(out_path).expect("cannot create output file");

    let mut written = 0usize;
    let mut skipped = 0usize;
    for line in std::io::BufReader::new(words).lines() {
        let line = line.expect("readable line");
        let word = line.trim();
        if word.is_empty() {
            continue;
        }
        match hangulize_rs::english_ipa_for_corpus(word) {
            Ok(ipa) if !ipa.is_empty() => {
                writeln!(out, "{word}\t{ipa}").expect("write must succeed");
                written += 1;
            }
            Ok(_) => {
                eprintln!("skip (empty phonemes): {word}");
                skipped += 1;
            }
            Err(err) => {
                eprintln!("skip ({err}): {word}");
                skipped += 1;
            }
        }
    }
    eprintln!("done: {written} written, {skipped} skipped");
}
