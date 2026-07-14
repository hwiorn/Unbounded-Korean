const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

pub fn decompose_char(ch: char) -> Option<(char, char, Option<char>)> {
    let code = ch as u32;
    if !(S_BASE..S_BASE + S_COUNT).contains(&code) {
        return None;
    }
    let s_index = code - S_BASE;
    let l = char::from_u32(L_BASE + s_index / N_COUNT)?;
    let v = char::from_u32(V_BASE + (s_index % N_COUNT) / T_COUNT)?;
    let t_index = s_index % T_COUNT;
    let t = if t_index == 0 {
        None
    } else {
        Some(char::from_u32(T_BASE + t_index)?)
    };
    Some((l, v, t))
}

pub fn decompose(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if let Some((l, v, t)) = decompose_char(ch) {
            out.push(l);
            out.push(v);
            if let Some(t) = t {
                out.push(t);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn compose_jamo(l: char, v: char, t: Option<char>) -> Option<char> {
    let l_index = l as u32 - L_BASE;
    let v_index = v as u32 - V_BASE;
    if l_index >= L_COUNT || v_index >= V_COUNT {
        return None;
    }
    let t_index = match t {
        Some(t) => {
            let idx = t as u32 - T_BASE;
            if idx == 0 || idx >= T_COUNT {
                return None;
            }
            idx
        }
        None => 0,
    };
    char::from_u32(S_BASE + (l_index * V_COUNT + v_index) * T_COUNT + t_index)
}

pub fn compose(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if is_vowel(ch) {
            out.push('ᄋ');
            i += 1;
            continue;
        }
        if is_lead(ch) && i + 1 < chars.len() && is_vowel(chars[i + 1]) {
            let tail = if i + 2 < chars.len() && is_tail(chars[i + 2]) {
                Some(chars[i + 2])
            } else {
                None
            };
            if let Some(syl) = compose_jamo(ch, chars[i + 1], tail) {
                out.push(syl);
                i += if tail.is_some() { 3 } else { 2 };
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

pub fn is_lead(ch: char) -> bool {
    ('\u{1100}'..='\u{1112}').contains(&ch)
}

pub fn is_vowel(ch: char) -> bool {
    ('\u{1161}'..='\u{1175}').contains(&ch)
}

pub fn is_tail(ch: char) -> bool {
    ('\u{11A8}'..='\u{11C2}').contains(&ch)
}

pub fn is_syllable(ch: char) -> bool {
    ('\u{AC00}'..='\u{D7A3}').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decomposes_and_composes_hangul() {
        assert_eq!(decompose("한글"), "한글");
        assert_eq!(compose("한글"), "한글");
    }
}
