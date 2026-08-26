//! Phoneme-to-Hangul (P2G) conversion. Takes the pre-tokenized phoneme list produced
//! by `sosap`'s decoder (each element is exactly one phoneme symbol from the FST's
//! output symbol table — the same simplified-IPA alphabet
//! `hangulize_rs::english_ipa_for_corpus`/`scripts/convert_cmudict_to_ipa.py`
//! established, since that's what the model was trained on) and composes it into
//! Hangul.
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
    /// A consonant that Korean's own phonemization marked as taking the neutral
    /// vowel ㅡ as its own syllable nucleus (signaled by the training data's "ɯ"
    /// token immediately following it — see `tokens_to_units`), rather than
    /// attaching to whichever vowel comes next. Unlike a plain `Consonant`, this
    /// renders immediately as its own syllable instead of queuing in `pending` for
    /// the following vowel to claim as an onset.
    NeutralSyllable(char),
}

fn unit_for(token: &str) -> Option<Unit> {
    Some(match token {
        "æ" => Unit::Vowel('ㅐ'),
        "ɛ" | "ə" | "e" | "ᵻ" => Unit::Vowel('ㅔ'),
        "ɜ" | "ʌ" | "ɔ" | "ɚ" | "ɝ" => Unit::Vowel('ㅓ'),
        "ɑ" | "a" => Unit::Vowel('ㅏ'),
        "i" | "ɪ" => Unit::Vowel('ㅣ'),
        "u" | "ʊ" => Unit::Vowel('ㅜ'),
        "o" | "oʊ" => Unit::Vowel('ㅗ'),
        "ɡ" | "g" => Unit::Consonant('ㄱ'),
        "k" => Unit::Consonant('ㅋ'),
        "t" => Unit::Consonant('ㅌ'),
        "d" => Unit::Consonant('ㄷ'),
        "p" => Unit::Consonant('ㅍ'),
        "b" => Unit::Consonant('ㅂ'),
        "f" => Unit::Consonant('ㅍ'),
        "v" => Unit::Consonant('ㅂ'),
        "s" | "θ" => Unit::Consonant('ㅅ'),
        "z" | "ð" => Unit::Consonant('ㅈ'),
        "ʃ" => Unit::Consonant('ㅅ'),
        "ʒ" => Unit::Consonant('ㅈ'),
        "tʃ" => Unit::Consonant('ㅊ'),
        "dʒ" => Unit::Consonant('ㅈ'),
        "h" => Unit::Consonant('ㅎ'),
        "m" => Unit::Consonant('ㅁ'),
        "n" => Unit::Consonant('ㄴ'),
        "ŋ" => Unit::Consonant('ㅇ'),
        "l" | "ɫ" | "r" | "ɹ" => Unit::Consonant('ㄹ'),
        "w" => Unit::Glide('W'),
        "j" => Unit::Glide('Y'),
        // Diphthongs arrive as one token from the decoder; push the first component
        // here and the second as a following plain Vowel unit (see
        // `diphthong_second_vowel`), mirroring hangulize-rs's `english_units` fix.
        "eɪ" => Unit::Vowel('ㅔ'),
        "aɪ" => Unit::Vowel('ㅏ'),
        "aʊ" => Unit::Vowel('ㅏ'),
        "ɔɪ" => Unit::Vowel('ㅗ'),
        _ => return None,
    })
}

fn diphthong_second_vowel(token: &str) -> Option<char> {
    match token {
        "eɪ" | "aɪ" | "ɔɪ" => Some('ㅣ'),
        "aʊ" => Some('ㅜ'),
        _ => None,
    }
}

fn tokens_to_units<S: AsRef<str>>(phonemes: &[S]) -> Vec<Unit> {
    let mut units = Vec::with_capacity(phonemes.len());
    for token in phonemes {
        let token = token.as_ref();
        // "ɯ" is not a phoneme in its own right -- it's the training-data signal
        // (Korean's own neutral coda-filler vowel, preserved by
        // examples/hangul_answer_to_ipa_corpus.rs's tokenizer) that the consonant
        // immediately before it takes ㅡ as its own syllable, rather than attaching
        // to whatever vowel comes next ("USA" 유에스에이 needs its middle S to stand
        // alone this way; P2G's normal onset-maximization would otherwise attach it
        // to the following 에이).
        if token == "ɯ" {
            if let Some(Unit::Consonant(c)) = units.last().copied() {
                *units.last_mut().unwrap() = Unit::NeutralSyllable(c);
            }
            continue;
        }
        let Some(unit) = unit_for(token) else {
            continue;
        };
        units.push(unit);
        if let Some(second) = diphthong_second_vowel(token) {
            units.push(Unit::Vowel(second));
        }
    }
    let units = collapse_geminate_consonants(units);
    collapse_syllabic_schwa_l(units)
}

/// A decoder can emit the same consonant phoneme twice in a row where English
/// orthography has a doubled letter but only one actual sound (observed with an
/// earlier, less accurate decoder: "hello" as "hɛlloʊ" — a real /l/-/l/ duplication
/// artifact, not a genuine geminate consonant cluster, since English doesn't
/// phonemically distinguish gemination). Collapsing adjacent identical consonants
/// avoids a spurious extra syllable like "헤르로" instead of "헬로".
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

/// English words ending in a syllabic consonant spelled "-Cəl" (apple, table, little)
/// drop the schwa in Korean loanword convention: the consonant before the schwa and
/// the trailing /l/ merge into one "consonant + ㅡ + ㄹ" syllable instead of the schwa
/// getting its own syllable (애플, not 애펠). Ported from hangulize-rs's
/// `collapse_syllabic_schwa_l`.
fn collapse_syllabic_schwa_l(units: Vec<Unit>) -> Vec<Unit> {
    let mut out = Vec::with_capacity(units.len());
    for (i, unit) in units.iter().copied().enumerate() {
        let syllabic_l = matches!(unit, Unit::Vowel('ㅔ'))
            && i > 0
            && i + 1 == units.len().saturating_sub(1)
            && matches!(units[i - 1], Unit::Consonant(_))
            && matches!(units[i + 1], Unit::Consonant('ㄹ'));
        if !syllabic_l {
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

pub fn phonemes_to_hangul<S: AsRef<str>>(phonemes: &[S]) -> String {
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
            Unit::NeutralSyllable(c) => {
                if !pending.is_empty() {
                    out.push_str(&render_stray_consonants(&pending));
                    pending.clear();
                }
                out.push(compose_syllable(c, 'ㅡ', None));
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
                    } else if matches!(after.first(), Some('ㄹ')) && after.len() > 1 {
                        // ㄹ followed by another consonant, with a vowel further
                        // ahead: ㄹ becomes THIS syllable's coda ("LG" 엘지, not
                        // 에르지 -- leaving all of `after` for the next vowel's
                        // onset-pop would strand ㄹ as its own 르 syllable instead).
                        // The single-ㄹ case above already doubles it into the next
                        // syllable's onset too when nothing else is queued; here the
                        // next consonant(s) queue normally instead.
                        out.push(compose_syllable(onset, vowel, Some('ㄹ')));
                        pending = after[1..].to_vec();
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

    fn tokens(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn composes_simple_cvc_word() {
        assert_eq!(phonemes_to_hangul(&tokens(&["h", "ɛ", "l", "oʊ"])), "헬로");
    }

    #[test]
    fn repairs_consecutive_consonants_from_a_dropped_vowel() {
        assert_eq!(phonemes_to_hangul(&tokens(&["s", "k", "t"])), "스크트");
    }

    #[test]
    fn repairs_consecutive_vowels_from_a_dropped_consonant() {
        assert_eq!(phonemes_to_hangul(&tokens(&["a", "i"])), "아이");
    }

    #[test]
    fn collapses_doubled_consonant_decoder_artifact() {
        assert_eq!(
            phonemes_to_hangul(&tokens(&["h", "ɛ", "l", "l", "oʊ"])),
            "헬로"
        );
    }

    #[test]
    fn collapses_syllabic_schwa_l_ending() {
        assert_eq!(phonemes_to_hangul(&tokens(&["æ", "p", "ə", "l"])), "애플");
    }

    #[test]
    fn maps_v_to_bieup_not_pieup() {
        // "v" must render as ㅂ (video -> 비디오, seven -> 세븐), not ㅍ (which is only
        // for "f" — the two were previously merged into one arm, which is wrong for
        // v-containing words like the NAVER brand-name training entry).
        assert_eq!(phonemes_to_hangul(&tokens(&["n", "eɪ", "v", "ɝ"])), "네이버");
    }

    #[test]
    fn reconstructs_explicit_double_l_from_korean_answer_derived_training_data() {
        // korean_go_ipa.tsv derives phonemes from the real Hangul answer, so an
        // intervocalic /l/ written across two syllables (마일리지) comes through as
        // two adjacent 'l' tokens, not the single phoneme collapse_geminate_consonants
        // expects from a genuine English word. Confirms the existing pipeline already
        // handles this: collapse reduces it to one 'l', and the onset/vowel loop's own
        // ㄹㄹ-doubling (for an intervocalic L, per 외래어 표기법 6항 rule 2) regenerates
        // the second syllable from that single L, landing on the same answer either way.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["m", "a", "i", "l", "l", "i", "dʒ", "i"])),
            "마일리지"
        );
    }

    #[test]
    fn stands_a_neutral_syllable_consonant_alone_between_two_vowels() {
        // "USA" (유에스에이): korean_go_ipa.tsv's "ɯ" marker after the middle S means
        // it takes its own syllable (스) instead of P2G's normal onset-maximization
        // attaching it to the following 에이 (which would give 유에세이).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["j", "u", "e", "s", "ɯ", "e", "i"])),
            "유에스에이"
        );
    }

    #[test]
    fn a_neutral_syllable_consonant_still_lets_an_earlier_consonant_become_a_coda() {
        // "text" (텍스트): raw phonemization is t-e-k-s-ɯ-t-ɯ. The K has no "ɯ" of
        // its own, so it must still become 텍's coda, not a bare "크" syllable --
        // confirms the neutral-syllable marker on S doesn't disturb the ordinary
        // final-cluster-splitting path for the K before it.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["t", "e", "k", "s", "ɯ", "t", "ɯ"])),
            "텍스트"
        );
    }

    #[test]
    fn an_l_before_another_consonant_becomes_a_coda_not_a_stray_syllable() {
        // "LG" (엘지): a coda-eligible ㄹ immediately followed by another consonant,
        // with a vowel further ahead, must become THIS syllable's coda (엘) -- the
        // general onset-cluster fallback (pop the last pending consonant as the next
        // onset, strand earlier ones as their own ㅡ-vowel syllables) would otherwise
        // strand ㄹ as a spurious "르" syllable, giving 에르지 instead.
        assert_eq!(phonemes_to_hangul(&tokens(&["e", "l", "dʒ", "i"])), "엘지");
    }

    #[test]
    fn world_is_unaffected_by_the_l_coda_fix() {
        // Regression guard: "world" (월드) already relies on split_final_cluster's
        // word-final path (ㄹ followed by ㄷ with no vowel after at all), a different
        // branch entirely from the one the LG fix touches -- confirms the two don't
        // collide.
        assert_eq!(phonemes_to_hangul(&tokens(&["w", "ɝ", "l", "d"])), "월드");
    }
}
