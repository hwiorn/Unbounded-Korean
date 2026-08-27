//! Standalone Hangul syllable composer (lead + vowel + optional tail -> one syllable
//! block), independent of hangulize-rs's internals. Uses the standard Unicode Hangul
//! Syllables formula: base + (lead_index * 588) + (vowel_index * 28) + tail_index.

const LEADS: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];
const VOWELS: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];
const TAILS: [char; 28] = [
    '\0', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ',
    'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// Composes one Hangul syllable block from a lead consonant, a vowel, and an optional
/// tail consonant. Falls back to returning the lead jamo alone if `lead`/`vowel` are
/// not in the standard 19/21-jamo sets (should not happen with the P2G table's output,
/// but this must not panic on unexpected input).
pub fn compose_syllable(lead: char, vowel: char, tail: Option<char>) -> char {
    let (Some(l), Some(v)) = (
        LEADS.iter().position(|&c| c == lead),
        VOWELS.iter().position(|&c| c == vowel),
    ) else {
        return lead;
    };
    let t = tail
        .and_then(|tc| TAILS.iter().position(|&c| c == tc))
        .unwrap_or(0);
    let code = 0xAC00 + (l * 588) + (v * 28) + t;
    char::from_u32(code as u32).unwrap_or(lead)
}

/// Decomposes one Hangul syllable block into its lead, vowel, and optional tail --
/// the inverse of `compose_syllable`. Returns `None` for a `char` outside the
/// precomposed Hangul Syllables block (U+AC00..=U+D7A3), e.g. plain ASCII or a jamo
/// that isn't part of a composed syllable.
pub fn decompose_syllable(ch: char) -> Option<(char, char, Option<char>)> {
    let code = ch as u32;
    if !(0xAC00..=0xD7A3).contains(&code) {
        return None;
    }
    let idx = code - 0xAC00;
    let lead = LEADS[(idx / (21 * 28)) as usize];
    let vowel = VOWELS[((idx % (21 * 28)) / 28) as usize];
    let tail_idx = (idx % 28) as usize;
    let tail = if tail_idx == 0 {
        None
    } else {
        Some(TAILS[tail_idx])
    };
    Some((lead, vowel, tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_a_syllable_into_lead_vowel_and_tail() {
        assert_eq!(decompose_syllable('헤'), Some(('ㅎ', 'ㅔ', None)));
        assert_eq!(decompose_syllable('영'), Some(('ㅇ', 'ㅕ', Some('ㅇ'))));
        assert_eq!(decompose_syllable('x'), None);
    }

    #[test]
    fn composes_simple_syllable_without_tail() {
        assert_eq!(compose_syllable('ㅎ', 'ㅔ', None), '헤');
    }

    #[test]
    fn composes_syllable_with_tail() {
        assert_eq!(compose_syllable('ㅇ', 'ㅕ', Some('ㅇ')), '영');
    }

    #[test]
    fn unknown_lead_falls_back_without_panicking() {
        assert_eq!(compose_syllable('x', 'ㅏ', None), 'x');
    }
}
