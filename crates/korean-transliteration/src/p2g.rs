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
    /// A real, standard IPA syllable-boundary mark (the "." in e.g. Wiktionary's
    /// /ˈdʌb.(ə)l.juː/ for "double-u") -- `reverse::hangul_to_phonemes` emits one
    /// between every pair of Hangul syllables it encodes. Without it, a plain
    /// phoneme stream carries no signal at all for whether a consonant right
    /// before a vowel/glide belongs to the syllable before it or the one after --
    /// most English words want the latter ("neuron" 뉴런, "new" 뉴), which is why
    /// that's every other branch's default, but a written answer can deliberately
    /// want the former (the "W" letter-name "더블유"'s own ㄹ must stay put, not
    /// flow into "유"; "MAD" spelled out as letters, "엠에이디", needs its ㅁ to
    /// stay in "엠", not flow into "에"). `Boundary` doesn't need any dedicated
    /// handling in the lookahead loops below: it simply isn't a `Consonant` or a
    /// `Vowel`, so every "collect trailing consonants" / "is a vowel coming next"
    /// check already stops at it for free, exactly as if it were the end of the
    /// word -- the one place it needs an explicit match arm is
    /// `phonemes_to_hangul`'s own top-level dispatch, which must advance past it
    /// without emitting anything.
    Boundary,
}

fn unit_for(token: &str) -> Option<Unit> {
    Some(match token {
        "æ" => Unit::Vowel('ㅐ'),
        "ɛ" | "e" | "ᵻ" => Unit::Vowel('ㅔ'),
        // "ə" (the unstressed schwa) is ARPABET AH0 -- the same base vowel as
        // stressed "ʌ" (AH1/AH2), just reduced. crates/g2pk's established
        // ARPABET->jamo table maps AH regardless of stress to the same jamo
        // (ᅥ, U+1165); grouping schwa with ɛ/e instead gave it a different
        // vowel quality than its own stressed form uses.
        "ɜ" | "ʌ" | "ə" | "ɚ" | "ɝ" => Unit::Vowel('ㅓ'),
        "ɑ" | "a" => Unit::Vowel('ㅏ'),
        "i" | "ɪ" => Unit::Vowel('ㅣ'),
        "u" | "ʊ" => Unit::Vowel('ㅜ'),
        // "ɔ" (AO, as in "ball"/"call"/"talk"/"fall"/"hall") is its own vowel
        // quality, distinct from ʌ/ə/ɜ/ɚ/ɝ -- confirmed against korean_go.tsv's
        // official answers (볼/콜/토크/폴/홀, all ㅗ, never ㅓ). Grouped with
        // "o"/"oʊ" since it renders the same way; see resolve_onset_vowel's
        // Glide('W') arm for the one place a preceding "w" changes this
        // (water/walk 워터/워크, not 오터/오크).
        "o" | "oʊ" | "ɔ" => Unit::Vowel('ㅗ'),
        "ɡ" | "g" => Unit::Consonant('ㄱ'),
        "k" => Unit::Consonant('ㅋ'),
        "t" => Unit::Consonant('ㅌ'),
        // American English flaps an intervocalic /t/ to a voiced tap -- misaki's
        // English IPA (eng_ipa.tsv) transcribes this surface realization as "ɾ"
        // (distinct from a genuine /d/, which it still spells "d": "water" w ɔ ɾ
        // ɚ vs "wedding" w ɛ d ɪ ŋ). Korean loanword convention follows the
        // underlying spelling, not the flap, so this still renders as ㅌ (water
        // 워터, city 시티, little 리틀) -- unmapped, it was silently dropped by
        // `unit_for`'s catch-all `_ => None`, corrupting ~24,000 corpus entries.
        "ɾ" => Unit::Consonant('ㅌ'),
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
        "l" | "ɫ" => Unit::Consonant('ㄹ'),
        // American /r/ (unlike /l/) vanishes entirely in Korean loanword
        // convention whenever it isn't immediately followed by a vowel --
        // confirmed against korean_go.tsv's official answers: "market" 마켓
        // (not 마르켓/말켓), "Harvard" 하버드 (not 할바드), "Cardiff" 카디프,
        // "party" 파티 -- a coda-position /r/ contributes nothing, not even a
        // stray syllable, unlike /l/ in the same position ("self" 셀프,
        // "world" 월드, which keep their ㄹ coda). Tracked as its own marker
        // (matching the 'W'/'Y' glide-placeholder convention) rather than the
        // real 'ㄹ' jamo so is_tail_consonant and render_stray_consonants can
        // tell it apart from a genuine /l/ and drop it instead of rendering a
        // coda or stray syllable; resolve_onset_vowel converts it to a normal
        // 'ㄹ' onset in the one case it doesn't vanish -- immediately before a
        // vowel ("hero" 히어로).
        "r" | "ɹ" => Unit::Consonant('R'),
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
    // Parallel to `units`: whether the unit at the same index came from a
    // literal "ə" token, not merely from unit_for's merged Vowel('ㅔ') target
    // (which "ɛ"/"e"/"ᵻ" also produce) -- see collapse_syllabic_schwa_l, which
    // needs to tell a real schwa apart from an ordinary /ɛ/ or /e/ that just
    // happens to render as the same jamo.
    let mut is_schwa = Vec::with_capacity(phonemes.len());
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
        if token == "." {
            units.push(Unit::Boundary);
            is_schwa.push(false);
            continue;
        }
        let Some(unit) = unit_for(token) else {
            continue;
        };
        units.push(unit);
        is_schwa.push(token == "ə");
        if let Some(second) = diphthong_second_vowel(token) {
            units.push(Unit::Vowel(second));
            is_schwa.push(false);
        }
    }
    collapse_syllabic_schwa_l(units, &is_schwa)
}

/// English words ending in a syllabic consonant spelled "-Cəl" (apple, table, little)
/// drop the schwa in Korean loanword convention: the consonant before the schwa and
/// the trailing /l/ merge into one "consonant + ㅡ + ㄹ" syllable instead of the schwa
/// getting its own syllable (애플, not 애펄). Ported from hangulize-rs's
/// `collapse_syllabic_schwa_l`.
///
/// `is_schwa[i]` must be checked alongside `units[i]` -- unit_for merges "ə" into
/// the same Unit::Vowel('ㅓ') target as "ʌ"/"ɜ"/"ɔ"/"ɚ"/"ɝ" (they're the same base
/// vowel; see unit_for's comment), but only a literal schwa drops out this way. An
/// ordinary stressed /ʌ/ before a word-final L is a real vowel that keeps its own
/// syllable ("gull" 걸, not 그르) -- checking only the merged jamo would collapse
/// those too.
fn collapse_syllabic_schwa_l(units: Vec<Unit>, is_schwa: &[bool]) -> Vec<Unit> {
    let mut out = Vec::with_capacity(units.len());
    for (i, unit) in units.iter().copied().enumerate() {
        let syllabic_l = matches!(unit, Unit::Vowel('ㅓ'))
            && is_schwa[i]
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
        (OnsetCandidate::Glide('W'), 'ㅐ') => ('ㅇ', 'ㅙ'),
        (OnsetCandidate::Glide('W'), 'ㅔ') => ('ㅇ', 'ㅞ'),
        (OnsetCandidate::Glide('W'), 'ㅓ') => ('ㅇ', 'ㅝ'),
        // Korean has no separate "wo" compound distinct from ㅗ itself, so a
        // "w" immediately before what would otherwise render as ㅗ (whether
        // from "ɔ", "o", or "oʊ") takes the closest existing compound instead
        // -- confirmed for "water"/"walk" 워터/워크 (not 오터/오크), both AO
        // (ɔ) same as "ball"'s ㅗ when there's no preceding "w".
        (OnsetCandidate::Glide('W'), 'ㅗ') => ('ㅇ', 'ㅝ'),
        (OnsetCandidate::Glide('W'), 'ㅣ') => ('ㅇ', 'ㅟ'),
        (OnsetCandidate::Glide('W'), 'ㅜ') => ('ㅇ', 'ㅜ'),
        (OnsetCandidate::Glide('Y'), 'ㅏ') => ('ㅇ', 'ㅑ'),
        // "Shanks" 섕(ㅅ+ㅒ+ㅇ), "Chamblee" 섐(ㅅ+ㅒ+ㅁ): korean_go.tsv's own
        // established answers for these, the same Y+æ pattern "shadow" 섀
        // already uses without a glide onset in front of it -- these are just
        // the first words this table needed it for with one. Missing this arm
        // meant reverse::hangul_to_phonemes had no way to encode ㅒ at all, so
        // both answers failed round-trip verification and silently fell back
        // to a lower-priority source's plain-ㅐ spelling (생크스/챔블리) instead.
        (OnsetCandidate::Glide('Y'), 'ㅐ') => ('ㅇ', 'ㅒ'),
        // ㅓ and ㅔ produce DIFFERENT compounds (여 vs 예) -- previously merged
        // into 'ㅖ' for both, which mis-rendered every Y+ㅓ syllable ("passion"
        // -> 션 needs 여, not 예).
        (OnsetCandidate::Glide('Y'), 'ㅓ') => ('ㅇ', 'ㅕ'),
        (OnsetCandidate::Glide('Y'), 'ㅔ') => ('ㅇ', 'ㅖ'),
        (OnsetCandidate::Glide('Y'), 'ㅗ') => ('ㅇ', 'ㅛ'),
        (OnsetCandidate::Glide('Y'), 'ㅜ') => ('ㅇ', 'ㅠ'),
        (OnsetCandidate::Glide('Y'), 'ㅣ') => ('ㅇ', 'ㅣ'),
        // The one case a coda-position-vanishing 'R' marker (see unit_for)
        // doesn't vanish: immediately before a vowel, it's a genuine onset
        // ("hero" 히어로) and renders as a normal ㄹ.
        (OnsetCandidate::Consonant('R'), vowel) => ('ㄹ', vowel),
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

/// Whether `ch` is a plain stop/fricative that combines with a following liquid
/// into one real English onset cluster (pl-/bl-/gl-/kl-/fl-/sl-, pr-/br-/gr-/kr-/
/// fr-/tr-/dr-) -- see `split_onset_liquid_cluster`'s own doc comment for why that
/// distinction matters here, separately from `is_tail_consonant`'s (which is about
/// which single consonants can be a coda at all, not which consonant PAIRS form a
/// valid English onset).
fn is_onset_cluster_obstruent(ch: char) -> bool {
    matches!(ch, 'ㅍ' | 'ㅂ' | 'ㄱ' | 'ㅋ' | 'ㅌ' | 'ㄷ' | 'ㅅ')
}

/// Splits a 2-consonant cluster immediately before a vowel into [the obstruent's
/// own neutral syllable] + [what the upcoming vowel should claim as its onset],
/// but only when the cluster is a genuine English onset cluster (obstruent +
/// liquid) that real English syllabification keeps together at the START of a
/// syllable -- unlike a pair such as Fonteyn's "nt", which splits ACROSS a
/// syllable boundary (n stays as the coda of the syllable before it, t becomes the
/// next syllable's onset; see `a_leading_consonant_in_a_cluster_becomes_a_coda_
/// before_the_next_vowel`'s own test). Without this distinction, the general
/// coda-peeling logic wrongly treated EVERY 2-consonant cluster the Fonteyn way,
/// corrupting any word with a real onset cluster mid-word: "class" -> 크래스
/// (should be 클래스), "program" -> 프록램 (should be 프로그램), "fabric" ->
/// 팹릭 (should be 패브릭).
///
/// A literal 'ㄹ' (real /l/) comes back doubled -- once as this new syllable's own
/// batchim, once still in the remainder for the next vowel to ALSO claim as an
/// onset ("class" 클래스, "black" 블랙) -- the same doubled-liquid shape Korean
/// convention already uses when a source word's own spelling repeats an
/// intervocalic /l/ (see `unit_for`'s "l"/"ɫ" comment), just triggered here by
/// cluster shape instead of two identical trained tokens. The 'R' marker (real
/// American /r/, which `unit_for`'s own comment says never becomes a batchim at
/// all) instead leaves this new syllable bare and hands only 'R' onward -- it
/// resolves to a normal ㄹ onset once `resolve_onset_vowel` sees it immediately
/// before the next vowel, the same "hero" 히로 rule already in place ("program"
/// 프로그램, "brake" 브레이크).
///
/// Returns `None` for anything else (not exactly 2 elements, or a first element
/// that isn't a real onset-forming obstruent), leaving the caller's existing
/// coda-peeling logic untouched -- including the doubled-'ㄹ'-'ㄹ' case ("mileage"
/// 마일리지), since 'ㄹ' itself is a liquid, not an obstruent.
fn split_onset_liquid_cluster(cluster: &[char]) -> Option<(char, Vec<char>)> {
    let [first, second] = cluster else {
        return None;
    };
    if !is_onset_cluster_obstruent(*first) {
        return None;
    }
    match second {
        'ㄹ' => Some((*first, vec!['ㄹ'])),
        'R' => Some((*first, vec!['R'])),
        _ => None,
    }
}

// 'ㄷ' and 'ㅅ' are deliberately excluded: a word-final /d/ or /s/ is never a
// batchim in Korean loanword convention -- each is always its own full
// syllable ("salad" 샐러드, "guard" 가드, "wood" 우드, "word" 워드, "card"
// 카드, "kid" 키드 for /d/; "gas" 가스, "glass" 글라스, "miss" 미스, "bus"
// 버스, "class" 클래스, "kiss" 키스 for /s/ -- 6/6 korean_go.tsv answers each,
// no counter-examples found for either). Excluding them here routes to
// render_stray_consonants' default ㅡ-vowel-syllable path instead of a coda.
//
// 'ㅌ'/'ㄱ'(g)/'ㅍ'(p)/'ㅂ' are NOT excluded despite similarly mixed evidence
// ("net"/"let"/"set"/"bat"/"hit" all full-syllable vs "Gosset"/"Witt"/
// "pivot" batchim for /t/; korean_go.tsv even carries two different answers
// for the same word, "web" 웨브 and 웹) -- unlike /d/ and /s/, these
// genuinely have no consistent rule to extract; which one an established
// loanword picked appears to be per-word history, the same kind of
// irreducible ambiguity as schwa placement (see unit_for's "ə" comment).
fn is_tail_consonant(ch: char) -> bool {
    matches!(
        ch,
        'ㄱ' | 'ㅋ' | 'ㄴ' | 'ㅌ' | 'ㄹ' | 'ㅁ' | 'ㅂ' | 'ㅍ' | 'ㅇ'
    )
}

fn as_tail(ch: char) -> char {
    match ch {
        'ㅋ' => 'ㄱ',
        'ㅍ' => 'ㅂ',
        'ㅌ' => 'ㅅ',
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
///
/// ㅈ/ㅊ (from dʒ/tʃ) are one exception: Korean loanword convention gives a
/// stranded word-final affricate ㅣ instead of ㅡ ("message" 메시지, "language"
/// 랭귀지, "package" 패키지, "orange" 오렌지 -- all end in 지, never 즈), unlike
/// every other consonant class (스,트,드,크,그,프,브 etc. all correctly use ㅡ).
///
/// The 'R' marker (see unit_for) is the other exception: an American /r/ that
/// ends up here (never immediately followed by a vowel -- otherwise it would
/// already have become a real ㄹ onset via resolve_onset_vowel) contributes
/// nothing at all, not even a syllable ("market" 마켓, not 마르켓; "star" 스타,
/// not 스타르).
fn render_stray_consonants(consonants: &[char]) -> String {
    consonants
        .iter()
        .filter_map(|&ch| match ch {
            'R' => None,
            'W' => Some('우'),
            'Y' => Some('이'),
            'ㅈ' | 'ㅊ' => Some(compose_syllable(ch, 'ㅣ', None)),
            ch => Some(compose_syllable(ch, 'ㅡ', None)),
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
            Unit::Boundary => {
                // Every lookahead below already stops here on its own (see this
                // variant's own doc comment) -- `pending` is guaranteed already
                // flushed by the time a `Boundary` is reached as a top-level
                // unit, so there's nothing to do but step past it.
                i += 1;
            }
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
                // A neutral syllable renders immediately, but consonants right
                // after it still need somewhere to land: a trailing cluster
                // before another vowel gives its leading (coda-eligible) member
                // to THIS syllable as a coda ("Claiborne" 클레이번's 클-coda +
                // 레-onset, from a genuinely doubled ㄹ), and a trailing cluster
                // at the word's end (no vowel following) does the same via
                // split_final_cluster ("little" 리틀, not 리트르). A single
                // consonant before another vowel is left alone (it becomes that
                // vowel's onset instead), matching the analogous rule for a
                // real vowel below.
                let mut j = i + 1;
                let mut after = Vec::new();
                while j < units.len() {
                    if let Unit::Consonant(cc) = units[j] {
                        after.push(cc);
                        j += 1;
                    } else {
                        break;
                    }
                }
                // Same "consonant immediately before a 'j' (Y) glide isn't a coda
                // candidate" un-consume as the Vowel branch below (see its own
                // comment) -- without this, a NeutralSyllable directly followed by
                // e.g. "s j e" ("Gottsched" 고트셰트's 셰) stranded the 's' as its
                // own bare 스 syllable instead of leaving it for 셰 to claim as its
                // true onset (고트스예트).
                if matches!(units.get(j), Some(Unit::Glide('Y'))) && after.pop().is_some() {
                    j -= 1;
                }
                let next_is_vowel = j < units.len() && matches!(units[j], Unit::Vowel(_));
                if next_is_vowel && after.len() > 1 {
                    if let (Some(tail), rest) = split_final_cluster(&after) {
                        out.push(compose_syllable(c, 'ㅡ', Some(tail)));
                        pending = rest.to_vec();
                        i = j;
                        continue;
                    }
                }
                if !next_is_vowel && !after.is_empty() {
                    let (tail, rest) = split_final_cluster(&after);
                    out.push(compose_syllable(c, 'ㅡ', tail));
                    if !rest.is_empty() {
                        out.push_str(&render_stray_consonants(rest));
                    }
                    i = j;
                    continue;
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
                let (mut onset, vowel) = resolve_onset_vowel(onset, vowel);
                // A plain consonant directly before a glide ("n" before "j" in
                // "new") is the syllable's real onset -- resolve_onset_vowel's
                // glide arms always place the placeholder ㅇ onset there, which is
                // only correct when nothing precedes the glide ("USA"'s 유).
                // Reclaim it from `pending` instead of letting it strand as its
                // own stray ㅡ-vowel syllable ("neuron" 뉴런, not 느율런).
                if matches!(onset_char, Some('W') | Some('Y')) {
                    if let Some(&last) = pending.last() {
                        if last != 'W' && last != 'Y' {
                            onset = if last == 'R' { 'ㄹ' } else { last };
                            pending.pop();
                        }
                    }
                }
                if !pending.is_empty() {
                    // "class" k+l+æ+s, "black" b+l+æ+k: pending holds exactly the
                    // leading obstruent (e.g. ㅋ/ㅂ) once the liquid onset_char
                    // above was already popped as this vowel's own onset -- an
                    // onset-cluster shape, not a genuine coda, so it must render
                    // WITH the doubled 'ㄹ' as its batchim (클/블), not as a bare
                    // stray syllable (크/브) -- see split_onset_liquid_cluster's
                    // own doc comment. "brake"/"program"'s obstruent+'R' never
                    // doubles (real /r/ has no batchim at all), so that stays bare.
                    if pending.len() == 1
                        && is_onset_cluster_obstruent(pending[0])
                        && matches!(onset_char, Some('ㄹ') | Some('R'))
                    {
                        let batchim = (onset_char == Some('ㄹ')).then_some('ㄹ');
                        out.push(compose_syllable(pending[0], 'ㅡ', batchim));
                    } else {
                        out.push_str(&render_stray_consonants(&pending));
                    }
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
                // A consonant immediately followed by a 'j' (Y) glide isn't a
                // coda candidate at all -- Korean's own compound vowels
                // ㅑㅕㅛㅠㅖㅒ decompose into this exact "j" + base-vowel shape
                // (see korean_phonemizer's medial_to_ipa), so this consonant is
                // really the onset of a SAME-syllable palatalized vowel
                // ("passion" 패션's `s` before `j`+ʌ forming 션, not an `s`
                // coda plus a stray "예"/"야" syllable from the orphaned
                // glide+vowel; "Cugnot" 퀴뇨's `n` before `j`+o forming 뇨).
                // Un-consume it so the ordinary Consonant/Glide/Vowel handling
                // above picks it up fresh, the same path "new"/"queen" already
                // use to combine a plain consonant with a following glide.
                //
                // A 'w' (W) glide is deliberately NOT included here: unlike
                // 'j', a preceding consonant before "w" is genuinely ambiguous
                // between belonging to a same-syllable onset-glide cluster and
                // being an ordinary coda of the syllable before an unrelated
                // w-initial syllable ("Olwen" 올웬's `l` really is 올's coda,
                // not part of a "lw" onset -- there's no reliable signal in a
                // flat phoneme stream to tell the two apart, and 'w' cases like
                // this are common enough among the hsl-derived languages that
                // guessing "combine" by default would trade a real regression
                // for the 'j' pattern's more common gain).
                if matches!(units.get(j), Some(Unit::Glide('Y'))) && after.pop().is_some() {
                    j -= 1;
                }
                let next_is_vowel = j < units.len() && matches!(units[j], Unit::Vowel(_));

                if next_is_vowel {
                    if let Some((obstruent, next_onset)) = split_onset_liquid_cluster(&after) {
                        // "program" p+r+oʊ+g+r+æ+m, "fabric" f+æ+b+r+ɪ+k: the
                        // trailing g+r/b+r is a real English onset cluster, not a
                        // cross-syllable pair like Fonteyn's n+t -- this syllable
                        // takes NO coda from it at all, the obstruent gets its own
                        // neutral syllable, and what's left (doubled 'ㄹ', or bare
                        // 'R') becomes the NEXT vowel's onset. See
                        // split_onset_liquid_cluster's own doc comment.
                        out.push(compose_syllable(onset, vowel, None));
                        let batchim = (next_onset == ['ㄹ']).then_some('ㄹ');
                        out.push(compose_syllable(obstruent, 'ㅡ', batchim));
                        pending = next_onset;
                        i = j;
                        continue;
                    }
                    let peeled = if after.len() > 1 {
                        match split_final_cluster(&after) {
                            (Some(tail), rest) => Some((tail, rest)),
                            (None, _) => None,
                        }
                    } else {
                        None
                    };
                    if let Some((tail, rest)) = peeled {
                        // A leading consonant in a cluster, with more consonants
                        // still ahead and a vowel further ahead still, becomes
                        // THIS syllable's coda instead of stranding into
                        // `pending` alongside the rest ("Fonteyn" 폰테인, not
                        // 포느테인 -- the trailing n+t cluster's n must become
                        // 폰's coda, not get flushed as its own 느 syllable once
                        // t claims the next onset). Generalizes the old ㄹ-only
                        // rule ("LG" 엘지) and the genuinely-doubled-liquid case
                        // ("mileage" 마일리지, after == ['ㄹ', 'ㄹ']) to any
                        // tail-eligible leading consonant.
                        out.push(compose_syllable(onset, vowel, Some(tail)));
                        pending = rest.to_vec();
                    } else {
                        // Either a single consonant (never gets a coda here --
                        // it becomes only the next syllable's onset, since a
                        // source /l/ or /r/ that was never doubled must not be
                        // forced into one: "neuron" 뉴런, not 뉼런) or a cluster
                        // whose leading consonant isn't coda-eligible at all.
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
        assert_eq!(phonemes_to_hangul(&tokens(&["b", "ʊ", "k"])), "북");
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
        // "hello" 헬로: this is a genuine doubled ㄹ (헬's coda + 로's onset), the
        // same case reconstructs_explicit_double_l_... below covers, not actually
        // a decoder artifact in this pipeline's Hangul-answer-derived training
        // data -- kept as a regression guard now that genuinely doubled
        // consonants are never collapsed at all (see
        // reconstructs_a_genuine_double_consonant_that_is_not_a_liquid below).
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
        // two adjacent 'l' tokens -- both real, both preserved (see
        // a_leading_consonant_in_a_cluster_becomes_a_coda_before_the_next_vowel):
        // the first becomes 일's coda, the second becomes 리's onset.
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

    #[test]
    fn a_consonant_immediately_before_a_glide_becomes_the_syllables_true_onset() {
        // "new" (뉴): pending holds [ㄴ, Y] when the vowel 'u' arrives.
        // pending.pop() only recovers the glide, so the plain consonant before it
        // was being stranded as its own stray syllable (느율 instead of 뉴).
        assert_eq!(phonemes_to_hangul(&tokens(&["n", "j", "u"])), "뉴");
        // "queen" (퀸): same bug with the 'w' glide (크윈 instead of 퀸).
        assert_eq!(phonemes_to_hangul(&tokens(&["k", "w", "i", "n"])), "퀸");
    }

    #[test]
    fn newton_and_newport_get_the_reclaimed_onset_correctly() {
        assert_eq!(
            phonemes_to_hangul(&tokens(&["n", "j", "u", "t", "ʌ", "n"])),
            "뉴턴"
        );
        assert_eq!(
            phonemes_to_hangul(&tokens(&["n", "j", "u", "p", "o", "t", "ɯ"])),
            "뉴포트"
        );
    }

    #[test]
    fn a_single_intervocalic_liquid_is_not_forced_into_a_double_coda() {
        // "neuron" is really /ˈn(j)ʊrɒn/ -- an American /r/, not /l/ -- so this
        // uses the "r" token (Consonant('R')'s own reclaim-before-a-vowel path,
        // exercised separately below by an_r_immediately_before_a_vowel_is_
        // still_a_real_onset) rather than a plain "l". Korean 뉴런 has no ㄹ coda
        // at all, only 런's onset, but the old "after == ['ㄹ']" branch
        // unconditionally doubled ANY single intervocalic liquid into a coda +
        // matching next onset, which is only correct for a liquid that really
        // was doubled in the source (an English intervocalic /l/, e.g. "mileage"
        // 마일리지 below) -- not for a single, undoubled /l/ or /r/ ("neuron"
        // 뉼런 was wrong).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["n", "j", "u", "r", "ʌ", "n"])),
            "뉴런"
        );
    }

    #[test]
    fn a_neutral_syllable_still_gets_a_coda_from_a_genuinely_doubled_liquid_after_it() {
        // "Claiborne" 클레이번: korean_go_ipa.tsv gives "k ɯ l l e i b ʌ n" -- the
        // neutral-syllable K (클's own ㅡ nucleus) is immediately followed by a
        // real doubled ㄹ (클's coda + 레's onset). NeutralSyllable used to render
        // immediately with no coda slot, stranding the first ㄹ as its own stray
        // "르" syllable once the second ㄹ claimed the next vowel's onset (크르레
        // instead of 클레).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["k", "ɯ", "l", "l", "e", "i", "b", "ʌ", "n"])),
            "클레이번"
        );
        // "Hockley" 호클리: "h o k ɯ l l i" -- same pattern, mid-word.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["h", "o", "k", "ɯ", "l", "l", "i"])),
            "호클리"
        );
    }

    #[test]
    fn a_real_e_before_a_word_final_l_keeps_its_own_syllable() {
        // "cell" 셀, "bell" 벨: unit_for merges "ə"/"ɛ"/"e" into the same
        // Vowel('ㅔ') target, but collapse_syllabic_schwa_l must only drop a
        // literal schwa -- these use "e" (a real vowel), not "ə", so the
        // vowel must survive (스르/브르 was the bug: the vowel silently
        // vanished, leaving only two consonants with nothing between them).
        assert_eq!(phonemes_to_hangul(&tokens(&["s", "e", "l"])), "셀");
        assert_eq!(phonemes_to_hangul(&tokens(&["b", "e", "l"])), "벨");
    }

    #[test]
    fn reconstructs_a_genuine_double_consonant_that_is_not_a_liquid() {
        // "ammeter" 암미터: a genuine double /m/ (am-coda + me-onset) must
        // survive just like a doubled ㄹ does -- collapsing any repeated
        // consonant used to also erase real non-liquid doubles (아미터).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["a", "m", "m", "i", "t", "ʌ"])),
            "암미터"
        );
    }

    #[test]
    fn y_glide_plus_eo_produces_yeo_not_ye() {
        // resolve_onset_vowel used to merge 'ㅓ' and 'ㅔ' into the same
        // compound 'ㅖ' (ye) for a Y glide onset; they're different vowels --
        // 'ㅓ' must produce 'ㅕ' (yeo) instead.
        assert_eq!(phonemes_to_hangul(&tokens(&["j", "ʌ"])), "여");
    }

    #[test]
    fn y_glide_plus_ae_produces_the_yae_compound() {
        // "Shanks" 섕: korean_go.tsv's own established answer, the same Y+æ
        // pattern "shadow" 섀 already uses without a preceding onset consonant
        // -- missing this arm meant reverse::hangul_to_phonemes had no way to
        // encode ㅒ at all, so "섕크스"/"섐블리" failed round-trip verification
        // and silently fell back to a lower-priority plain-ㅐ answer instead.
        assert_eq!(phonemes_to_hangul(&tokens(&["s", "j", "æ", "ŋ"])), "섕");
    }

    #[test]
    fn a_consonant_before_a_w_glide_still_becomes_the_earlier_syllables_coda() {
        // "Olwen" 올웬: unlike the 'j'-glide case below, a consonant right
        // before a 'w' glide keeps its old coda-of-the-preceding-syllable
        // behavior -- `l` becomes 올's coda, and `w`+`e` starts a fresh
        // syllable (웬) with no real onset consonant.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["o", "l", "w", "e", "n"])),
            "올웬"
        );
    }

    #[test]
    fn a_consonant_before_a_glide_still_forms_a_coda_syllable_correctly() {
        // "passion" 패션: the "s j ʌ n" tail is a consonant directly before a
        // glide, exactly like "new"'s "n j u" -- but here it follows a vowel
        // that already looked ahead for a coda cluster, so `s` used to get
        // swallowed into that lookahead and treated as 패's coda, stranding
        // the orphaned "j ʌ n" as its own onset-less syllable (팻옌 instead of
        // 패션). The lookahead must skip a consonant immediately followed by
        // a glide, leaving it to combine with the glide as this new
        // syllable's onset instead.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["p", "æ", "s", "j", "ʌ", "n"])),
            "패션"
        );
    }

    #[test]
    fn a_neutral_syllable_directly_before_a_glide_syllable_does_not_strand_its_onset() {
        // "Gottsched" 고트셰트: the neutral-syllable T (트's own ㅡ nucleus) is
        // directly followed by "s j e t ɯ" (셰트) -- the trailing 's' must be left
        // for 셰 to claim as its true onset, the same "consonant immediately
        // before a Y glide isn't a coda candidate" rule the Vowel branch already
        // applies (see a_consonant_before_a_glide_still_forms_a_coda_syllable_
        // correctly below). Previously stranded 's' as its own bare 스 syllable
        // (고트스예트) since NeutralSyllable's lookahead didn't know about glides.
        assert_eq!(
            phonemes_to_hangul(&tokens(&[
                "ɡ", "o", "t", "ɯ", "s", "j", "e", "t", "ɯ"
            ])),
            "고트셰트"
        );
    }

    #[test]
    fn a_neutral_syllable_takes_a_single_trailing_consonant_as_its_own_coda() {
        // "little" 리틀: the neutral-syllable T (틀's own ㅡ nucleus) is
        // followed by exactly one more consonant at the word's end -- it must
        // become THIS syllable's coda (틀), not a separate stray syllable
        // (리트르). The earlier fix only handled a trailing *pair* of ㄹ's
        // before another vowel; this generalizes it to any trailing cluster,
        // including a single consonant at the word's end.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["l", "i", "t", "ɯ", "l"])),
            "리틀"
        );
    }

    #[test]
    fn a_word_final_affricate_gets_an_i_not_a_neutral_vowel() {
        // "message" 메시지, not 메시즈: a stranded word-final ㅈ/ㅊ takes ㅣ, unlike
        // every other consonant class (스,트,드,크 etc. all correctly take ㅡ).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["m", "ɛ", "s", "ɪ", "dʒ"])),
            "메시지"
        );
    }

    #[test]
    fn ao_is_its_own_vowel_distinct_from_the_schwa_group() {
        // "ball"/"call"/"fall"/"hall" -- confirmed against korean_go.tsv's
        // official 볼/콜/폴/홀, all ㅗ, never ㅓ. (Not "talk": word-final /k/
        // has the same kind of irreducible per-word batchim-vs-syllable
        // ambiguity as /t/ -- "rock"/"look" 록/룩 vs "bank"/"pink" 뱅크/핑크 --
        // unrelated to this vowel fix, so it doesn't belong in this test.)
        assert_eq!(phonemes_to_hangul(&tokens(&["b", "ɔ", "l"])), "볼");
        assert_eq!(phonemes_to_hangul(&tokens(&["k", "ɔ", "l"])), "콜");
    }

    #[test]
    fn w_plus_ao_still_takes_the_wo_compound_like_w_plus_eo_does() {
        // "water" 워터: Korean has no separate "wo" compound distinct from ㅗ
        // itself, so a "w" before what would render as ㅗ takes the closest
        // existing compound (ㅝ) instead, same as it already does for a
        // genuine ʌ/ə-class vowel. (Not "walk": same word-final /k/ ambiguity
        // as above, unrelated to this fix.)
        assert_eq!(
            phonemes_to_hangul(&tokens(&["w", "ɔ", "t", "ɝ"])),
            "워터"
        );
    }

    #[test]
    fn an_r_before_another_consonant_vanishes_instead_of_becoming_a_coda() {
        // Confirmed against korean_go.tsv's official answers: unlike /l/ (which
        // keeps its coda in the same position -- "self" 셀프, "world" 월드),
        // American /r/ contributes nothing at all when it isn't immediately
        // followed by a vowel. Uses "e" rather than the literal schwa "ə" for
        // "market"'s middle vowel, and "party"'s R+consonant+VOWEL shape for
        // the second case, so this test isolates the /r/ rule from the
        // separate (and separately documented) schwa- and stop-consonant-
        // batchim ambiguities.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["m", "a", "ɹ", "k", "e", "t"])),
            "마켓"
        );
        assert_eq!(
            phonemes_to_hangul(&tokens(&["h", "a", "ɹ", "v", "ɝ", "d"])),
            "하버드"
        );
        assert_eq!(
            phonemes_to_hangul(&tokens(&["p", "a", "ɹ", "t", "i"])),
            "파티"
        );
    }

    #[test]
    fn a_word_final_r_alone_vanishes_without_a_coda_or_a_stray_syllable() {
        // "star"/"car" 스타/카, not 스타르/카르 or 스탈/칼.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["s", "t", "a", "ɹ"])),
            "스타"
        );
        assert_eq!(phonemes_to_hangul(&tokens(&["k", "a", "ɹ"])), "카");
    }

    #[test]
    fn a_word_final_d_is_always_a_full_syllable_never_a_batchim() {
        // "salad" 샐러드, "card" 카드: confirmed against 6/6 korean_go.tsv
        // answers with a word-final /d/, no counter-examples found -- unlike
        // /t/, which genuinely has no consistent rule (net/let/set/bat/hit
        // all full-syllable vs Gosset/Witt/pivot batchim). "salad"'s /l/ is
        // pre-doubled (two 'l' tokens) the way build_training_corpus.py's
        // double_intervocalic_l already leaves a raw-IPA source's genuine
        // intervocalic /l/ -- this test isolates the /d/ rule, not that one.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["s", "æ", "l", "l", "ə", "d"])),
            "샐러드"
        );
        assert_eq!(phonemes_to_hangul(&tokens(&["k", "a", "ɹ", "d"])), "카드");
    }

    #[test]
    fn a_word_final_s_is_always_a_full_syllable_never_a_batchim() {
        // "bus" 버스, "kiss" 키스: confirmed against 6/6 korean_go.tsv answers
        // with a word-final /s/, no counter-examples found. (Not "gas": its
        // established 가스 uses ㅏ for what cmudict transcribes as /æ/, a
        // separate, already-known vowel-mapping irregularity unrelated to
        // this /s/ rule.)
        assert_eq!(phonemes_to_hangul(&tokens(&["b", "ʌ", "s"])), "버스");
        assert_eq!(phonemes_to_hangul(&tokens(&["k", "ɪ", "s"])), "키스");
    }

    #[test]
    fn an_r_immediately_before_a_vowel_is_still_a_real_onset() {
        // The one case an 'R' marker doesn't vanish: immediately before a
        // vowel, it's a real syllable onset, same as "market"'s middle
        // consonant would be if it were followed by a vowel instead of "k".
        assert_eq!(phonemes_to_hangul(&tokens(&["h", "i", "ɹ", "o"])), "히로");
    }

    #[test]
    fn schwa_matches_crates_g2pks_established_ah_mapping() {
        // crates/g2pk's ARPABET->jamo table sends AH to ᅥ regardless of stress
        // (english.rs: `"AH" | "ER" => "\u{1165}"`) -- "ə" (unstressed AH0) is the
        // same base vowel as stressed "ʌ" (AH1/AH2), which already mapped to ㅓ.
        // "mileage" from cmudict's primary (schwa) pronunciation -- doubled 'l'
        // (see build_training_corpus.py's double_intervocalic_l) -- now matches
        // crates/g2pk's own documented, accepted answer for this exact word
        // (english.rs's converts_mileage_using_cmudicts_primary_pronunciation).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["m", "aɪ", "l", "l", "ə", "dʒ"])),
            "마일러지"
        );
    }

    #[test]
    fn a_word_final_tieut_becomes_a_siot_coda_not_a_stray_syllable() {
        // "Gosset" 고셋: word-final "t" (ㅌ) must become ㅅ 받침 like ㄷ already
        // does -- is_tail_consonant was missing 'ㅌ' even though as_tail already
        // knew how to convert it, so this always fell through to a stray "트"
        // syllable instead (고세트).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["ɡ", "o", "s", "e", "t"])),
            "고셋"
        );
    }

    #[test]
    fn a_leading_consonant_in_a_cluster_becomes_a_coda_before_the_next_vowel() {
        // "Fonteyn" 폰테인: the "n t" cluster between o and e must split into
        // 폰's coda (n) and 테's onset (t), not strand into `pending` together
        // and flush n as its own stray "느" syllable once t claims the onset
        // (포느테인). Generalizes the old ㄹ-only ("LG" 엘지) coda-carry rule to
        // any tail-eligible leading consonant.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["p", "o", "n", "t", "e", "i", "n"])),
            "폰테인"
        );
    }

    #[test]
    fn a_leading_obstruent_liquid_onset_cluster_does_not_split_like_fonteyns() {
        // "class" k+l+æ+s: unlike Fonteyn's "nt" (a genuine cross-syllable pair),
        // "kl" is one real English onset cluster -- the k must get its own
        // neutral syllable WITH the doubled 'ㄹ' as its batchim (클), not a bare
        // stray syllable (크), confirmed against korean_go.tsv's own established
        // 클래스 (cited by is_tail_consonant's own /s/-never-batchim comment).
        assert_eq!(phonemes_to_hangul(&tokens(&["k", "l", "æ", "s"])), "클래스");
        // "black" b+l+æ+k: same doubled-ㄹ pattern, confirmed against
        // korean_go.tsv's 블랙.
        assert_eq!(phonemes_to_hangul(&tokens(&["b", "l", "æ", "k"])), "블랙");
    }

    #[test]
    fn a_trailing_obstruent_liquid_onset_cluster_does_not_split_like_fonteyns() {
        // "program" p+r+oʊ+g+r+æ+m: the trailing "gr" is a real onset cluster,
        // not Fonteyn's cross-syllable "nt" -- 오 must take NO coda from it at
        // all (프로, not 프록), confirmed against korean_go.tsv's 프로그램. Real
        // American /r/ never doubles into a batchim (unit_for's own comment), so
        // this stays bare -- unlike the literal-'ㄹ' case above.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["p", "r", "oʊ", "g", "r", "æ", "m"])),
            "프로그램"
        );
        // "fabric" f+æ+b+r+ɪ+k: same bare-obstruent pattern, confirmed against
        // korean_go.tsv's 패브릭 (not the "팹릭" a coda-peeling read would give).
        assert_eq!(
            phonemes_to_hangul(&tokens(&["f", "æ", "b", "r", "ɪ", "k"])),
            "패브릭"
        );
    }

    #[test]
    fn maps_the_flap_to_tieut_not_silently_dropping_it() {
        // "water" w ɔ ɾ ɚ -- an unmapped phoneme used to vanish entirely
        // (워어 instead of 워터), not just render with the wrong jamo.
        assert_eq!(phonemes_to_hangul(&tokens(&["w", "ɔ", "ɾ", "ɚ"])), "워터");
    }

    #[test]
    fn a_genuinely_doubled_liquid_from_the_korean_answer_still_doubles() {
        // Regression guard for reconstructs_explicit_double_l_from_korean_answer_
        // derived_training_data's underlying mechanism, using un-collapsed input
        // directly: two REAL adjacent 'l' tokens (Korean 일 coda + 리 onset) must
        // still produce two syllables, now via the general "ㄹ followed by another
        // consonant" coda-carry path rather than the deleted forced-redouble branch.
        assert_eq!(
            phonemes_to_hangul(&tokens(&["m", "a", "i", "l", "l", "i", "dʒ", "i"])),
            "마일리지"
        );
    }
}
