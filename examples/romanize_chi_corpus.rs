// Converts a (Hanzi, Hangul-answer) TSV into (Pinyin, Hangul-answer), so Chinese's
// (word, Hangul) pairs can feed the same hangul_answer_to_ipa_corpus tokenizer every
// other language uses. Hanzi is logographic -- a character doesn't decompose into
// letter-sized sound units the way an alphabet does, so Phonetisaurus's grapheme-to-
// phoneme alignment can't learn anything from it directly (unlike Dutch, Italian,
// German, Spanish, whose native spelling already IS phonetic-ish). Pinyin gives every
// Hanzi word an alphabetic proxy spelling that alignment can work with, mirroring how
// hangulize-rs's own "pinyin" translit (crates/hangulize-rs/src/lib.rs's
// transliterate_pinyin) already romanizes Chinese before its rewrite-rule engine runs.
//
// Usage: romanize_chi_corpus <input.tsv> <output.tsv>
//
// Input format: tab-separated, word<TAB>...<TAB>hangul (word in column 0, Hangul
// answer in the last column, matching hsl_seed.tsv's lang\tword\thangul).

use pinyin::ToPinyin;
use std::env;
use std::fs;
use std::io::Write;

fn hanzi_to_pinyin(word: &str) -> Option<String> {
    let mut out = String::new();
    for ch in word.chars() {
        let syllable = ch.to_pinyin()?.plain();
        out.push_str(syllable);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
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
        match hanzi_to_pinyin(word) {
            Some(pinyin) => {
                writeln!(out, "{pinyin}\t{hangul}").expect("write output");
                written += 1;
            }
            None => {
                eprintln!("skip (no pinyin found): {word}");
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
    fn converts_hanzi_to_concatenated_plain_pinyin() {
        assert_eq!(hanzi_to_pinyin("毛澤東"), Some("maozedong".to_string()));
    }

    #[test]
    fn returns_none_for_a_word_with_no_pinyin_readings() {
        assert_eq!(hanzi_to_pinyin(""), None);
        assert_eq!(hanzi_to_pinyin("ABC"), None);
    }
}
