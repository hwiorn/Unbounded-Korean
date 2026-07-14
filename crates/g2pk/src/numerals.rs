use once_cell::sync::Lazy;
use regex::Regex;

const BOUND_NOUNS: &str = "군데 권 개 그루 닢 대 두 마리 모 모금 뭇 발 발짝 방 번 벌 보루 살 수 술 시 쌈 움큼 정 짝 채 척 첩 축 켤레 톨 통";

static BOUND_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"([\d][\d,]*)([가-힣]+)/B").unwrap());

pub fn process_num(num: &str, sino: bool) -> String {
    let num = num.replace(',', "");
    if num == "0" {
        return "영".to_string();
    }
    if !sino && num == "20" {
        return "스무".to_string();
    }

    let names = ["", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
    let modifiers = [
        "", "한", "두", "세", "네", "다섯", "여섯", "일곱", "여덟", "아홉",
    ];
    let decimals = [
        "", "열", "스물", "서른", "마흔", "쉰", "예순", "일흔", "여든", "아흔",
    ];
    let units = [
        "", "십", "백", "천", "만", "십", "백", "천", "억", "십", "백", "천", "조", "십", "백",
        "천",
    ];

    let digits: Vec<usize> = num
        .chars()
        .filter_map(|ch| ch.to_digit(10).map(|d| d as usize))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for (idx, digit) in digits.iter().enumerate() {
        let pos = digits.len() - idx - 1;
        if *digit == 0 {
            if pos % 4 == 0 {
                let recent = out.iter().rev().take(3).fold(String::new(), |mut acc, s| {
                    acc.insert_str(0, s);
                    acc
                });
                if recent.is_empty() {
                    out.push(String::new());
                }
            }
            continue;
        }
        let mut name = if sino {
            names[*digit].to_string()
        } else if pos == 0 {
            modifiers[*digit].to_string()
        } else if pos == 1 {
            decimals[*digit].to_string()
        } else {
            names[*digit].to_string()
        };
        if pos >= 1 && pos < units.len() {
            if !(pos == 1 && !sino) {
                name.push_str(units[pos]);
                for prefix in ["일십", "일백", "일천", "일만"] {
                    if name == prefix {
                        name = prefix[3..].to_string();
                    }
                }
            }
        }
        out.push(name);
    }
    out.concat()
}

pub fn convert_num(input: &str) -> String {
    let mut out = input.to_string();
    for cap in BOUND_RE.captures_iter(input).collect::<Vec<_>>() {
        let whole = cap.get(0).unwrap().as_str();
        let num = cap.get(1).unwrap().as_str();
        let noun = cap.get(2).unwrap().as_str();
        let sino = !BOUND_NOUNS.split_whitespace().any(|bn| bn == noun);
        let replacement = format!("{}{noun}/B", process_num(num, sino));
        out = out.replace(whole, &replacement);
    }
    let mut counters: Vec<&str> = BOUND_NOUNS.split_whitespace().collect();
    counters.sort_by_key(|counter| std::cmp::Reverse(counter.len()));
    for counter in counters {
        let re = Regex::new(&format!(r"([\d][\d,]*)({counter})")).unwrap();
        out = re
            .replace_all(&out, |caps: &regex::Captures<'_>| {
                format!("{}{}", process_num(&caps[1], false), &caps[2])
            })
            .to_string();
    }
    for (d, name) in [
        ('0', "영"),
        ('1', "일"),
        ('2', "이"),
        ('3', "삼"),
        ('4', "사"),
        ('5', "오"),
        ('6', "육"),
        ('7', "칠"),
        ('8', "팔"),
        ('9', "구"),
    ] {
        out = out.replace(d, name);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_sino_and_native_numbers() {
        assert_eq!(
            process_num("123,456,789", true),
            "일억이천삼백사십오만육천칠백팔십구"
        );
        assert_eq!(process_num("20", false), "스무");
        assert_eq!(convert_num("우리 3시/B 10분/B"), "우리 세시/B 십분/B");
        assert_eq!(convert_num("3개를"), "세개를");
    }
}
