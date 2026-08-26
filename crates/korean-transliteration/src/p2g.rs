//! Phoneme-to-Hangul (P2G) conversion. Takes the simplified-IPA phoneme string
//! produced by the trained Phonetisaurus G2P model (the same alphabet
//! `hangulize_rs::english_ipa_for_corpus` emits, since that's what the model was
//! trained on) and composes it into Hangul.
//!
//! The onset/vowel/glide/final-cluster composition rules here are ported from
//! `crates/hangulize-rs`'s `english_units`/`english_phoneme_word_to_hangul` (fixed
//! earlier in this session), not reinvented, so this crate doesn't regress behavior
//! that's already been validated against real words and acronyms.

use crate::hangul::compose_syllable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    /// A plain consonant onset/coda candidate.
    Consonant(char),
    /// The 'w'/'y' glide onsets, which combine with the following vowel into a
    /// compound vowel (와/워/워/워/워 etc.) rather than composing as a plain onset.
    Glide(char),
    Vowel(char),
}

fn unit_for(ch: char) -> Option<Unit> {
    Some(match ch {
        'æ' => Unit::Vowel('ㅐ'),
        'ɛ' | 'ə' | 'e' | 'ᵻ' => Unit::Vowel('ㅔ'),
        'ɜ' | 'ʌ' | 'ɔ' | 'ɚ' | 'ɝ' => Unit::Vowel('ㅓ'),
        'ɑ' | 'a' => Unit::Vowel('ㅏ'),
        'i' | 'ɪ' => Unit::Vowel('ㅣ'),
        'u' | 'ʊ' => Unit::Vowel('ㅜ'),
        'o' => Unit::Vowel('ㅗ'),
        'ɡ' | 'g' => Unit::Consonant('ㄱ'),
        'k' => Unit::Consonant('ㅋ'),
        't' => Unit::Consonant('ㅌ'),
        'd' => Unit::Consonant('ㄷ'),
        'p' => Unit::Consonant('ㅍ'),
        'b' => Unit::Consonant('ㅂ'),
        'f' | 'v' => Unit::Consonant('ㅍ'),
        's' | 'θ' => Unit::Consonant('ㅅ'),
        'z' | 'ð' => Unit::Consonant('ㅈ'),
        'ʃ' => Unit::Consonant('ㅅ'),
        'ʒ' => Unit::Consonant('ㅈ'),
        'h' => Unit::Consonant('ㅎ'),
        'm' => Unit::Consonant('ㅁ'),
        'n' => Unit::Consonant('ㄴ'),
        'ŋ' => Unit::Consonant('ㅇ'),
        'l' | 'ɫ' | 'r' | 'ɹ' => Unit::Consonant('ㄹ'),
        'w' => Unit::Glide('W'),
        'j' => Unit::Glide('Y'),
        _ => return None,
    })
}

/// Multi-codepoint symbols that must be matched before falling back to per-char
/// `unit_for`, in the same style as hangulize-rs's `english_units` scanner (this
/// crate's phoneme output is a continuous IPA-derived string, not whitespace-
/// delimited tokens, despite the training corpus being written with spaces between
/// them — Phonetisaurus's decoder does not preserve that separator).
const DIPHTHONGS: &[(&str, char, char)] = &[
    ("oʊ", 'ㅗ', '\0'),
    ("eɪ", 'ㅔ', 'ㅣ'),
    ("aɪ", 'ㅏ', 'ㅣ'),
    ("aʊ", 'ㅏ', 'ㅜ'),
    ("ɔɪ", 'ㅗ', 'ㅣ'),
];
const AFFRICATES: &[(&str, char)] = &[("tʃ", 'ㅊ'), ("dʒ", 'ㅈ')];

fn tokens_to_units(phonemes: &str) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut rest = phonemes;
    while !rest.is_empty() {
        if let Some(&(sym, first, second)) = DIPHTHONGS.iter().find(|(sym, ..)| rest.starts_with(sym)) {
            units.push(Unit::Vowel(first));
            if second != '\0' {
                units.push(Unit::Vowel(second));
            }
            rest = &rest[sym.len()..];
            continue;
        }
        if let Some(&(sym, jamo)) = AFFRICATES.iter().find(|(sym, _)| rest.starts_with(sym)) {
            units.push(Unit::Consonant(jamo));
            rest = &rest[sym.len()..];
            continue;
        }
        let ch = rest.chars().next().unwrap();
        if let Some(unit) = unit_for(ch) {
            units.push(unit);
        }
        rest = &rest[ch.len_utf8()..];
    }
    collapse_geminate_consonants(units)
}

/// The trained model sometimes emits the same consonant phoneme twice in a row where
/// English orthography has a doubled letter but only one actual sound (e.g. "hello"
/// decodes as "hɛlloʊ" — a real /l/-/l/ duplication artifact, not a genuine geminate
/// consonant cluster, since English doesn't phonemically distinguish gemination).
/// Collapsing adjacent identical consonants avoids spurious extra syllables like
/// "헤르로" instead of "헬로".
fn collapse_geminate_consonants(units: Vec<Unit>) -> Vec<Unit> {
    let mut out: Vec<Unit> = Vec::with_capacity(units.len());
    for unit in units {
        let is_repeat = matches!(
            (out.last(), unit),
            (Some(Unit::Consonant(a)), Unit::Consonant(b)) if *a == b
        );
        if !is_repeat {
            out.push(unit);
        }
    }
    out
}

/// Combines a glide onset ('W'/'Y' placeholders) with the following vowel into a
/// compound vowel, or leaves a plain consonant onset untouched.
fn resolve_onset_vowel(onset: OnsetCandidate, vowel: char) -> (char, char) {
    match (onset, vowel) {
        (OnsetCandidate::Glide('W'), 'ㅏ') => ('ㅇ', 'ㅘ'),
        (OnsetCandidate::Glide('W'), 'ㅐ' | 'ㅔ') => ('ㅇ', 'ㅞ'),
        (OnsetCandidate::Glide('W'), 'ㅓ') => ('ㅇ', 'ㅝ'),
        (OnsetCandidate::Glide('W'), 'ㅣ') => ('ㅇ', 'ㅟ'),
        (OnsetCandidate::Glide('W'), 'ㅜ') => ('ㅇ', 'ㅜ'),
        (OnsetCandidate::Glide('Y'), 'ㅏ') => ('ㅇ', 'ㅑ'),
        (OnsetCandidate::Glide('Y'), 'ㅓ' | 'ㅔ') => ('ㅇ', 'ㅖ'),
        (OnsetCandidate::Glide('Y'), 'ㅗ') => ('ㅇ', 'ㅛ'),
        (OnsetCandidate::Glide('Y'), 'ㅜ') => ('ㅇ', 'ㅠ'),
        (OnsetCandidate::Glide('Y'), 'ㅣ') => ('ㅇ', 'ㅣ'),
        (OnsetCandidate::Consonant(c), vowel) => (c, vowel),
        (OnsetCandidate::Glide(c), vowel) => (c, vowel), // unreachable in practice
        (OnsetCandidate::None, vowel) => ('ㅇ', vowel),
    }
}

#[derive(Clone, Copy)]
enum OnsetCandidate {
    None,
    Consonant(char),
    Glide(char),
}

fn final_blend(cluster: &[char]) -> Option<(char, char)> {
    match cluster {
        ['ㅍ', 'ㄹ'] | ['ㅂ', 'ㄹ'] | ['ㄱ', 'ㄹ'] | ['ㅋ', 'ㄹ'] => Some((cluster[0], cluster[1])),
        _ => None,
    }
}

fn is_tail_consonant(ch: char) -> bool {
    matches!(
        ch,
        'ㄱ' | 'ㅋ' | 'ㄴ' | 'ㄷ' | 'ㄹ' | 'ㅁ' | 'ㅂ' | 'ㅍ' | 'ㅅ' | 'ㅇ'
    )
}

fn as_tail(ch: char) -> char {
    match ch {
        'ㅋ' => 'ㄱ',
        'ㅍ' => 'ㅂ',
        'ㅌ' | 'ㄷ' => 'ㅅ',
        other => other,
    }
}

fn split_final_cluster(cluster: &[char]) -> (Option<char>, &[char]) {
    match cluster.split_first() {
        Some((&first, rest)) if is_tail_consonant(first) => (Some(as_tail(first)), rest),
        _ => (None, cluster),
    }
}

/// Renders leftover consonants that never found a vowel to attach to (either because
/// they trail the word or a run of consecutive consonants left extras after the first
/// one became an onset) as their own syllables with a neutral vowel (ㅡ) — the
/// `phoneme-gap-repaired` exception handling this crate's Allium contract requires.
fn render_stray_consonants(consonants: &[char]) -> String {
    consonants
        .iter()
        .map(|&ch| match ch {
            'W' => '우',
            'Y' => '이',
            ch => compose_syllable(ch, 'ㅡ', None),
        })
        .collect()
}

pub fn phonemes_to_hangul(phonemes: &str) -> String {
    let units = tokens_to_units(phonemes);
    let mut out = String::new();
    let mut pending: Vec<char> = Vec::new();
    let mut i = 0;
    while i < units.len() {
        match units[i] {
            Unit::Consonant(c) => {
                pending.push(c);
                i += 1;
            }
            Unit::Glide(c) => {
                pending.push(c);
                i += 1;
            }
            Unit::Vowel(vowel) => {
                let onset_char = pending.pop();
                let onset = match onset_char {
                    Some('W') => OnsetCandidate::Glide('W'),
                    Some('Y') => OnsetCandidate::Glide('Y'),
                    Some(c) => OnsetCandidate::Consonant(c),
                    None => OnsetCandidate::None,
                };
                let (onset, vowel) = resolve_onset_vowel(onset, vowel);
                if !pending.is_empty() {
                    out.push_str(&render_stray_consonants(&pending));
                    pending.clear();
                }

                let mut j = i + 1;
                let mut after = Vec::new();
                while j < units.len() {
                    if let Unit::Consonant(c) = units[j] {
                        after.push(c);
                        j += 1;
                    } else {
                        break;
                    }
                }
                let next_is_vowel = j < units.len() && matches!(units[j], Unit::Vowel(_));

                if next_is_vowel {
                    if after == ['ㄹ'] {
                        out.push(compose_syllable(onset, vowel, Some('ㄹ')));
                        pending.push('ㄹ');
                    } else {
                        out.push(compose_syllable(onset, vowel, None));
                        pending = after;
                    }
                } else if let Some((lead, tail)) = final_blend(&after) {
                    out.push(compose_syllable(onset, vowel, None));
                    out.push(compose_syllable(lead, 'ㅡ', Some(tail)));
                } else {
                    let (tail, rest) = split_final_cluster(&after);
                    out.push(compose_syllable(onset, vowel, tail));
                    if !rest.is_empty() {
                        out.push_str(&render_stray_consonants(rest));
                    }
                }
                i = j;
            }
        }
    }
    if !pending.is_empty() {
        out.push_str(&render_stray_consonants(&pending));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_simple_cvc_word() {
        // Continuous string, matching phonetisaurus-g2p's real output format (it does
        // not preserve the training corpus's whitespace token boundaries).
        assert_eq!(phonemes_to_hangul("hɛloʊ"), "헬로");
    }

    #[test]
    fn repairs_consecutive_consonants_from_a_dropped_vowel() {
        assert_eq!(phonemes_to_hangul("skt"), "스크트");
    }

    #[test]
    fn repairs_consecutive_vowels_from_a_dropped_consonant() {
        assert_eq!(phonemes_to_hangul("ai"), "아이");
    }

    #[test]
    fn collapses_doubled_consonant_decoder_artifact() {
        // The trained model emitted "hɛlloʊ" for "hello" (a genuine observed decoder
        // artifact — English "hello" has one /l/ sound, not two).
        assert_eq!(phonemes_to_hangul("hɛlloʊ"), "헬로");
    }
}
