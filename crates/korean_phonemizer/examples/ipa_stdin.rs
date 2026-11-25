use std::io::BufRead;

fn main() {
    for line in std::io::stdin().lock().lines() {
        let text = line.unwrap();
        match korean_phonemizer::phonemize_ko(&text) {
            Ok(out) => println!("{}", out.ipa),
            Err(err) => println!("ERR {err}"),
        }
    }
}
