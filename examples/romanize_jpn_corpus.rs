// Converts a (Kanji, Hangul-answer) TSV into (kana-reading, Hangul-answer), so
// Japanese's (word, Hangul) pairs can feed the same hangul_answer_to_ipa_corpus
// tokenizer every other language uses. Kanji is logographic -- like Hanzi, it doesn't
// decompose into letter-sized sound units, so Phonetisaurus's grapheme-to-phoneme
// alignment can't learn anything from it directly. hangulize_rs::kana_reading_for_corpus
// (the same Lindera/ipadic reading lookup hangulize-rs's own "furigana" translit mode
// uses) gives every kanji word a syllabic-alphabet proxy spelling that alignment can
// work with.
//
// A family+given name pair ("木村拓哉" -> "キムラ タクヤ") reads with a space at the
// name boundary, matching the Hangul answer's own space ("기무라 다쿠야") -- both sides'
// spaces are stripped here so the pair trains as one word, since
// hangul_answer_to_ipa_corpus's is_clean_word rejects multi-word entries the same way
// it does for e.g. Dutch's "Guido van Rossum".
//
// Usage: romanize_jpn_corpus <input.tsv> <output.tsv>
//
// Input format: tab-separated, word<TAB>...<TAB>hangul (word in column 0, Hangul
// answer in the last column, matching hsl_seed.tsv's lang\tword\thangul).

use std::env;
use std::fs;
use std::io::Write;

fn strip_spaces(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_path = args.get(1).expect("usage: <input.tsv> <output.tsv>");
    let output_path = args.get(2).expect("usage: <input.tsv> <output.tsv>");

    let content = fs::read_to_string(input_path).expect("read input");
    let mut out = fs::File::create(output_path).expect("create output");

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
        match hangulize_rs::kana_reading_for_corpus(word) {
            Ok(reading) if !reading.is_empty() => {
                writeln!(out, "{}\t{}", strip_spaces(&reading), strip_spaces(hangul))
                    .expect("write output");
                written += 1;
            }
            Ok(_) => {
                eprintln!("skip (empty reading): {word}");
                skipped += 1;
            }
            Err(e) => {
                eprintln!("skip (reading failed): {word} ({e})");
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
    fn strips_the_family_given_name_boundary_space() {
        assert_eq!(strip_spaces("キムラ タクヤ"), "キムラタクヤ");
        assert_eq!(strip_spaces("기무라 다쿠야"), "기무라다쿠야");
    }
}
