use crate::{Error, Result};
use regex::Regex;

const WORD_INTERNAL_RULE_IDS: [&str; 2] = ["19", "20"];
const BUILTIN_TABLE: &str = include_str!("resources/table.csv");
const BUILTIN_IDIOMS: &str = include_str!("resources/idioms.txt");

#[derive(Clone, Debug, Default)]
pub struct ResourceConfig {
    pub table_csv: Option<String>,
    pub idioms_txt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleEntry {
    pub from: String,
    pub to: String,
    pub rule_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledRuleEntry {
    pub regex: Regex,
    pub replacement: String,
}

#[derive(Clone, Debug)]
pub struct Resources {
    pub(crate) table: Vec<CompiledRuleEntry>,
    pub(crate) idioms: Vec<(Regex, String)>,
}

impl Resources {
    pub fn load(config: &ResourceConfig) -> Result<Self> {
        let table_src = config.table_csv.as_deref().unwrap_or(BUILTIN_TABLE);
        let idioms_src = config.idioms_txt.as_deref().unwrap_or(BUILTIN_IDIOMS);
        let table = parse_table(table_src)?
            .into_iter()
            .map(|entry| {
                let regex = Regex::new(&entry.from).map_err(|err| {
                    Error::InvalidResource(format!("invalid table pattern: {err}"))
                })?;
                Ok(CompiledRuleEntry {
                    regex,
                    replacement: entry.to.replace(r"\1", "$1"),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let idioms = parse_idioms(idioms_src)
            .into_iter()
            .map(|(from, to)| {
                let regex = Regex::new(&from)
                    .map_err(|err| Error::InvalidResource(format!("invalid idiom: {err}")))?;
                Ok((regex, to))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { table, idioms })
    }
}

pub fn parse_table(src: &str) -> Result<Vec<RuleEntry>> {
    let mut lines = src.lines();
    let header = lines
        .next()
        .ok_or_else(|| Error::InvalidResource("table is empty".to_string()))?;
    let onsets: Vec<&str> = header.split(',').collect();
    if onsets.len() < 2 {
        return Err(Error::InvalidResource(
            "table header has no onsets".to_string(),
        ));
    }

    let mut entries = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() != onsets.len() {
            return Err(Error::InvalidResource(format!(
                "table row has {} columns, expected {}",
                cols.len(),
                onsets.len()
            )));
        }
        let coda = cols[0];
        for (i, onset) in onsets.iter().enumerate().skip(1) {
            let cell = cols[i].trim();
            if cell.is_empty() {
                continue;
            }
            let (to, rule_ids) = if let Some(open) = cell.find('(') {
                let close = cell
                    .rfind(')')
                    .ok_or_else(|| Error::InvalidResource("unclosed rule id list".to_string()))?;
                (
                    cell[..open].to_string(),
                    cell[open + 1..close]
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect(),
                )
            } else {
                (cell.to_string(), Vec::new())
            };
            // 표준발음법 19·20항(ㄹ의 비음화, ㄴ↔ㄹ 유음화)과 ㄹ 초성이 ㄴ으로 바뀌는 칸은 한 단어
            // 안에서만 적용된다. 받침-초성 사이 공백 허용(`( ?)`)을 빼서 어절을 넘지 않게 한다.
            let word_internal = onset.ends_with('ᄅ')
                || rule_ids.iter().any(|id| WORD_INTERNAL_RULE_IDS.contains(&id.as_str()));
            let onset = if word_internal {
                onset.replacen("( ?)", "()", 1)
            } else {
                onset.to_string()
            };
            entries.push(RuleEntry {
                from: format!("{coda}{onset}"),
                to,
                rule_ids,
            });
        }
    }
    Ok(entries)
}

pub fn parse_idioms(src: &str) -> Vec<(String, String)> {
    src.lines()
        .filter_map(|line| {
            let stripped = line.split('#').next()?.trim();
            let (left, right) = stripped.split_once("===")?;
            Some((left.trim().to_string(), right.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_builtin_table() {
        let table = parse_table(BUILTIN_TABLE).unwrap();
        assert!(
            table
                .iter()
                .any(|entry| !entry.from.is_empty() && !entry.to.is_empty())
        );
    }

    #[test]
    fn custom_resources_override_builtin() {
        let resources = Resources::load(&ResourceConfig {
            table_csv: Some(",ᄀ\nᆨ,ᆨᄁ(1)\n".to_string()),
            idioms_txt: Some("abc===def\n".to_string()),
        })
        .unwrap();
        assert_eq!(resources.table[0].regex.as_str(), "ᆨᄀ");
        assert_eq!(resources.table[0].replacement, "ᆨᄁ");
        assert_eq!(resources.idioms[0].0.as_str(), "abc");
        assert_eq!(resources.idioms[0].1, "def");
    }

    fn table_output(text: &str) -> String {
        let resources = Resources::load(&ResourceConfig::default()).unwrap();
        crate::hangul::compose(&crate::rules::apply_table(
            crate::hangul::decompose(text),
            &resources.table,
        ))
    }

    #[test]
    fn liquid_and_rieul_onset_rules_do_not_cross_a_word_boundary() {
        assert_eq!(table_output("를 넣"), "를 넏");
        assert_eq!(table_output("물 나요"), "물 나요");
        assert_eq!(table_output("책 라디오"), "책 라디오");
        assert_eq!(table_output("옷 라디오"), "옫 라디오");
    }

    #[test]
    fn liquid_and_rieul_onset_rules_apply_inside_a_word() {
        assert_eq!(table_output("칼날"), "칼랄");
        assert_eq!(table_output("신라"), "실라");
        assert_eq!(table_output("막론"), "망논");
        assert_eq!(table_output("협력"), "혐녁");
    }
}
