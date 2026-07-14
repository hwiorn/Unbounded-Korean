use crate::resources::CompiledRuleEntry;
use once_cell::sync::Lazy;
use regex::Regex;

fn replace_all(input: &str, pat: &str, repl: &str) -> String {
    Regex::new(pat)
        .unwrap()
        .replace_all(input, repl)
        .to_string()
}

pub fn apply_special(mut input: String, descriptive: bool) -> String {
    input = replace_all(&input, "([ᄌᄍᄎ])ᅧ", "$1ᅥ");
    if descriptive {
        input = replace_all(&input, "([ᄀᄁᄃᄄㄹᄆᄇᄈᄌᄍᄎᄏᄐᄑᄒ])ᅨ", "$1ᅦ");
        input = input.replace("의/J", "에");
        input = replace_all(&input, r"(\Sᄋ)ᅴ", "$1ᅵ");
    } else {
        input = input.replace("/J", "");
    }
    input = replace_all(&input, "([ᄀᄁᄂᄃᄄᄅᄆᄇᄈᄉᄊᄌᄍᄎᄏᄐᄑᄒ])ᅴ", "$1ᅵ");
    for (pat, repl) in [
        ("([그])ᆮᄋ", "$1ᄉ"),
        ("([으])[ᆽᆾᇀᇂ]ᄋ", "$1ᄉ"),
        ("([으])[ᆿ]ᄋ", "$1ᄀ"),
        ("([으])[ᇁ]ᄋ", "$1ᄇ"),
        ("ᆰ/P([ᄀᄁ])", "ᆯᄁ"),
        ("([ᆲᆴ])/Pᄀ", "$1ᄁ"),
        ("([ᆲᆴ])/Pᄃ", "$1ᄄ"),
        ("([ᆲᆴ])/Pᄉ", "$1ᄊ"),
        ("([ᆲᆴ])/Pᄌ", "$1ᄍ"),
        ("([ᆫᆷ])/Pᄀ", "$1ᄁ"),
        ("([ᆫᆷ])/Pᄃ", "$1ᄄ"),
        ("([ᆫᆷ])/Pᄉ", "$1ᄊ"),
        ("([ᆫᆷ])/Pᄌ", "$1ᄍ"),
        ("ᆮᄋ([ᅵᅧ])", "ᄌ$1"),
        ("ᇀᄋ([ᅵᅧ])", "ᄎ$1"),
        ("ᆴᄋ([ᅵᅧ])", "ᆯᄎ$1"),
        ("ᆮᄒ([ᅵ])", "ᄎ$1"),
        ("ᆯ/E ᄀ", "ᆯ ᄁ"),
        ("ᆯ/E ᄃ", "ᆯ ᄄ"),
        ("ᆯ/E ᄇ", "ᆯ ᄈ"),
        ("ᆯ/E ᄉ", "ᆯ ᄊ"),
        ("ᆯ/E ᄌ", "ᆯ ᄍ"),
    ] {
        input = replace_all(&input, pat, repl);
    }
    for (from, to) in [
        ("ᆬ/Pᄀ", "ᆫᄁ"),
        ("ᆬ/Pᄃ", "ᆫᄄ"),
        ("ᆬ/Pᄉ", "ᆫᄊ"),
        ("ᆬ/Pᄌ", "ᆫᄍ"),
        ("ᆱ/Pᄀ", "ᆷᄁ"),
        ("ᆱ/Pᄃ", "ᆷᄄ"),
        ("ᆱ/Pᄉ", "ᆷᄊ"),
        ("ᆱ/Pᄌ", "ᆷᄍ"),
        ("ᆯ걸", "ᆯ껄"),
        ("ᆯ밖에", "ᆯ빠께"),
        ("ᆯ세라", "ᆯ쎄라"),
        ("ᆯ수록", "ᆯ쑤록"),
        ("ᆯ지라도", "ᆯ찌라도"),
        ("ᆯ지언정", "ᆯ찌언정"),
        ("ᆯ진대", "ᆯ찐대"),
    ] {
        input = input.replace(from, to);
    }
    input = Regex::new("(바)ᆲ($|[^ᄋᄒ])")
        .unwrap()
        .replace_all(&input, "$1ᆸ$2")
        .to_string();
    Regex::new("(너)ᆲ([ᄌᄍ]ᅮ|[ᄃᄄ]ᅮ)")
        .unwrap()
        .replace_all(&input, "$1ᆸ$2")
        .to_string()
}

pub(crate) fn apply_table(mut input: String, table: &[CompiledRuleEntry]) -> String {
    for entry in table {
        input = entry
            .regex
            .replace_all(&input, entry.replacement.as_str())
            .to_string();
    }
    input
}

pub fn apply_links(mut input: String) -> String {
    for (from, to) in [
        ("ᆨᄋ", "ᄀ"),
        ("ᆩᄋ", "ᄁ"),
        ("ᆫᄋ", "ᄂ"),
        ("ᆮᄋ", "ᄃ"),
        ("ᆯᄋ", "ᄅ"),
        ("ᆷᄋ", "ᄆ"),
        ("ᆸᄋ", "ᄇ"),
        ("ᆺᄋ", "ᄉ"),
        ("ᆻᄋ", "ᄊ"),
        ("ᆽᄋ", "ᄌ"),
        ("ᆾᄋ", "ᄎ"),
        ("ᆿᄋ", "ᄏ"),
        ("ᇀᄋ", "ᄐ"),
        ("ᇁᄋ", "ᄑ"),
        ("ᆪᄋ", "ᆨᄊ"),
        ("ᆬᄋ", "ᆫᄌ"),
        ("ᆰᄋ", "ᆯᄀ"),
        ("ᆱᄋ", "ᆯᄆ"),
        ("ᆲᄋ", "ᆯᄇ"),
        ("ᆳᄋ", "ᆯᄊ"),
        ("ᆴᄋ", "ᆯᄐ"),
        ("ᆵᄋ", "ᆯᄑ"),
        ("ᆹᄋ", "ᆸᄊ"),
        ("ᇂᄋ", "ᄋ"),
        ("ᆭᄋ", "ᄂ"),
        ("ᆶᄋ", "ᄅ"),
    ] {
        input = input.replace(from, to);
    }
    input
}

pub fn strip_markers(input: &str) -> String {
    static MARKERS: Lazy<Regex> = Lazy::new(|| Regex::new("/[PJEB]").unwrap());
    MARKERS.replace_all(input, "").to_string()
}

pub fn group_vowels(input: &str) -> String {
    input
        .replace('ᅢ', "ᅦ")
        .replace('ᅤ', "ᅨ")
        .replace('ᅫ', "ᅬ")
        .replace('ᅰ', "ᅬ")
}

pub fn annotate_by_tags(input: &str, tags: &[(String, String)]) -> String {
    let compact: String = input.chars().filter(|ch| *ch != ' ').collect();
    let token_text: String = tags.iter().map(|(token, _)| token.as_str()).collect();
    if compact != token_text {
        return input.to_string();
    }

    let mut tag_seq = String::new();
    for (token, tag) in tags {
        let mut chars = token.chars().peekable();
        while chars.next().is_some() {
            if chars.peek().is_some() {
                tag_seq.push('_');
            } else {
                tag_seq.push(map_pos(tag));
            }
        }
    }

    let mut tag_chars = tag_seq.chars();
    let mut out = String::new();
    for ch in input.chars() {
        out.push(ch);
        if ch == ' ' {
            continue;
        }
        let tag = tag_chars.next().unwrap_or('_');
        let decomposed = crate::hangul::decompose(&ch.to_string());
        let last = decomposed.chars().last();
        if ch == '의' && tag == 'J' {
            out.push_str("/J");
        } else if tag == 'E' && last == Some('ᆯ') {
            out.push_str("/E");
        } else if tag == 'V' && matches!(last, Some('ᆫ' | 'ᆬ' | 'ᆷ' | 'ᆱ' | 'ᆰ' | 'ᆲ' | 'ᆴ'))
        {
            out.push_str("/P");
        } else if tag == 'B' {
            out.push_str("/B");
        }
    }
    out
}

fn map_pos(pos: &str) -> char {
    if pos == "NNBC" {
        'B'
    } else {
        pos.chars().next().unwrap_or('_')
    }
}
