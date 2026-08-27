//! Literal, context-free reverse of `p2g`'s own tables: turns a human-verified
//! Hangul answer back into the same phoneme alphabet `p2g::phonemes_to_hangul`
//! consumes, syllable by syllable, with no Korean phonology applied at all (no
//! lateralization/nasalization/palatalization/tensification/position-dependent
//! voicing -- see `p2g`'s own module doc for why a *real pronunciation* engine like
//! `korean_phonemizer`/`g2pk::G2p` is the wrong tool here: it changes what the
//! written answer says, e.g. "월넛" -> "월럳", to match how a Korean speaker would
//! actually say it, which `p2g`'s forward direction can never undo since it has no
//! Korean phonology of its own to reverse).
//!
//! This is a genuine mathematical inverse of `p2g::phonemes_to_hangul`'s tables, not
//! an independent guess: every choice here (which phoneme a jamo reverses to, which
//! canonical pick to use when several phonemes map to the same jamo) is read
//! directly off `p2g`'s `unit_for`/`resolve_onset_vowel`/`is_tail_consonant`/
//! `as_tail` tables, so `p2g::phonemes_to_hangul(&hangul_to_phonemes(h).unwrap())
//! == h` holds for any syllable shape the forward table can produce at all. A
//! syllable shape forward can never produce (e.g. a written batchim ㄷ/ㅌ/ㅋ/ㅍ/ㅎ --
//! `p2g::as_tail` never emits those, only ㄱ/ㄴ/ㅅ/ㄹ/ㅁ/ㅂ/ㅇ) returns `None`: no
//! phoneme sequence could round-trip through it either way, so the caller should
//! skip that word rather than train on a lossy guess.

/// One vowel jamo's literal phoneme encoding.
enum VowelEncoding {
    /// A single phoneme token, e.g. ㅏ -> "a".
    Plain(&'static str),
    /// A glide onset ("w"/"j") plus its base-vowel phoneme, e.g. ㅘ -> ("w", "a").
    /// Matches `p2g::resolve_onset_vowel`'s Glide arms exactly -- see that
    /// function's own comments for why some compounds (ㅚ/ㅢ/ㅒ) have no forward
    /// arm at all and so can't appear here either.
    Glide(&'static str, &'static str),
    /// ㅡ: not a phoneme in `p2g`'s own table at all, only ever produced via the
    /// "ɯ" neutral-syllable marker immediately after a real onset consonant (see
    /// `p2g`'s `tokens_to_units`) -- requires a non-null lead.
    Neutral,
    /// No forward arm produces this vowel (ㅚ/ㅢ/ㅒ): the word can't round-trip.
    Unsupported,
}

fn lead_phoneme(lead: char) -> Option<&'static str> {
    // Mirrors `p2g::unit_for`'s Consonant arms, picking one canonical source
    // phoneme per jamo where several map to the same target (e.g. "z"/"ð"/"dʒ"
    // all -> ㅈ) -- the choice never affects round-tripping since every one of
    // them forward-renders to the identical jamo in every context `p2g` has.
    match lead {
        'ㄱ' => Some("ɡ"),
        'ㄴ' => Some("n"),
        'ㄷ' => Some("d"),
        'ㄹ' => Some("l"),
        'ㅁ' => Some("m"),
        'ㅂ' => Some("b"),
        'ㅅ' => Some("s"),
        'ㅇ' => None, // null onset -- not a phoneme, just "no consonant here"
        'ㅈ' => Some("dʒ"),
        'ㅊ' => Some("tʃ"),
        'ㅋ' => Some("k"),
        'ㅌ' => Some("t"),
        'ㅍ' => Some("p"),
        'ㅎ' => Some("h"),
        // Tense onsets (ㄲㄸㅃㅆㅉ): no arm in `unit_for` produces one of these as
        // a lead jamo at all -- unrepresentable, handled by the caller returning
        // `None` for the whole word.
        _ => None,
    }
}

fn vowel_encoding(vowel: char) -> VowelEncoding {
    use VowelEncoding::*;
    match vowel {
        'ㅏ' => Plain("a"),
        'ㅐ' => Plain("æ"),
        'ㅑ' => Glide("j", "a"),
        'ㅓ' => Plain("ʌ"),
        'ㅔ' => Plain("e"),
        'ㅕ' => Glide("j", "ʌ"),
        'ㅖ' => Glide("j", "e"),
        'ㅗ' => Plain("o"),
        'ㅘ' => Glide("w", "a"),
        'ㅙ' => Glide("w", "æ"),
        'ㅛ' => Glide("j", "o"),
        'ㅜ' => Plain("u"),
        'ㅝ' => Glide("w", "ʌ"),
        'ㅞ' => Glide("w", "e"),
        'ㅟ' => Glide("w", "i"),
        'ㅠ' => Glide("j", "u"),
        'ㅡ' => Neutral,
        'ㅣ' => Plain("i"),
        // ㅚ/ㅢ/ㅒ: no `resolve_onset_vowel`/`unit_for` arm produces these at all.
        _ => Unsupported,
    }
}

fn tail_phoneme(tail: char) -> Option<&'static str> {
    // Only the batchim jamo `p2g::as_tail` can actually OUTPUT -- see that
    // function: ㄱ/ㄴ/ㄹ/ㅁ/ㅂ/ㅇ pass through unchanged, ㅋ->ㄱ, ㅍ->ㅂ, ㅌ->ㅅ. A
    // written ㅅ batchim in this English-loanword domain can therefore only ever
    // represent a converted ㅌ (a real word-final /s/ is never spelled as a
    // batchim at all -- see `p2g::is_tail_consonant`'s own comment), so it must
    // reverse to "t", never "s". Any OTHER written batchim (ㄷ/ㅌ/ㅋ/ㅍ/ㅎ/ㅆ or a
    // cluster tail) is something the forward table can never produce, so it has
    // no faithful reverse either.
    match tail {
        'ㄱ' => Some("ɡ"),
        'ㄴ' => Some("n"),
        'ㅅ' => Some("t"),
        'ㄹ' => Some("l"),
        'ㅁ' => Some("m"),
        'ㅂ' => Some("b"),
        'ㅇ' => Some("ŋ"),
        _ => None,
    }
}

/// Turns a Hangul answer into the phoneme token sequence `p2g::phonemes_to_hangul`
/// would need to reproduce it exactly. Returns `None` if any syllable uses a jamo
/// shape the forward table has no way to produce (see the module doc) -- callers
/// should skip that word rather than train on a lossy guess.
pub fn hangul_to_phonemes(hangul: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    for ch in hangul.chars() {
        let (lead, vowel, tail) = crate::hangul::decompose_syllable(ch)?;
        let lead_ph = lead_phoneme(lead);
        if lead != 'ㅇ' && lead_ph.is_none() {
            return None; // tense/unsupported onset
        }
        match vowel_encoding(vowel) {
            VowelEncoding::Plain(v) => {
                if let Some(l) = lead_ph {
                    tokens.push(l.to_string());
                }
                tokens.push(v.to_string());
            }
            VowelEncoding::Glide(glide, base) => {
                if let Some(l) = lead_ph {
                    tokens.push(l.to_string());
                }
                tokens.push(glide.to_string());
                tokens.push(base.to_string());
            }
            VowelEncoding::Neutral => {
                let l = lead_ph?; // a null-onset ㅡ syllable has no representation
                tokens.push(l.to_string());
                tokens.push("ɯ".to_string());
            }
            VowelEncoding::Unsupported => return None,
        }
        if let Some(tail) = tail {
            tokens.push(tail_phoneme(tail)?.to_string());
        }
    }
    Some(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2g::phonemes_to_hangul;

    fn round_trips(hangul: &str) {
        let phonemes = hangul_to_phonemes(hangul)
            .unwrap_or_else(|| panic!("{hangul} has no literal phoneme encoding"));
        assert_eq!(
            phonemes_to_hangul(&phonemes),
            hangul,
            "{hangul} -> {phonemes:?} -> did not round-trip"
        );
    }

    #[test]
    fn round_trips_a_plain_cvc_word() {
        round_trips("커피");
    }

    #[test]
    fn round_trips_a_doubled_liquid_across_syllables() {
        round_trips("마일리지");
        round_trips("헬로");
    }

    #[test]
    fn round_trips_a_neutral_syllable_word() {
        round_trips("텍스트");
        round_trips("유에스에이");
    }

    #[test]
    fn round_trips_glide_compound_vowels() {
        round_trips("컴퓨터"); // 퓨 = ㅍ onset + ㅠ (j,u) glide vowel
        round_trips("패션"); // 션 = ㅅ onset + ㅕ (j,ʌ) glide vowel
        round_trips("워터"); // 워 = null onset + ㅝ (w,ʌ) glide vowel
    }

    #[test]
    fn round_trips_a_word_final_batchim() {
        round_trips("월드"); // ㄹ + ㄷ(full syllable) -- D is never a batchim
        round_trips("고셋"); // word-final T spelled as a ㅅ batchim
    }

    #[test]
    fn does_not_produce_a_lossy_guess_for_an_unrepresentable_batchim() {
        // "낫" (ㄴ+ㅏ+ㅅ): a native-Korean word choice, not an English-loanword
        // batchim `p2g::as_tail` would ever emit -- treated the same either way
        // since ㅅ only round-trips as a converted ㅌ; this just documents that a
        // genuinely unrepresentable shape returns `None` rather than a wrong guess.
        assert!(hangul_to_phonemes("힣").is_none()); // ㅎ+ㅣ+ㅎ: ㅎ batchim, no arm
    }

    #[test]
    fn walnut_does_not_lateralize_unlike_the_real_pronunciation_engine() {
        // The whole point of this module: "월넛" (the written korean_go.tsv
        // answer) must reverse-and-forward back to itself, unlike
        // `korean_phonemizer`'s real-speech pipeline, which turns it into "월럳"
        // via Korean's own ㄹ+ㄴ lateralization rule (see the module doc).
        round_trips("월넛");
    }
}
