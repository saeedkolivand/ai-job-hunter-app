//! Language-level parsing: which `(language, level)` pairs a text asserts.

use std::collections::HashMap;

/// A language level on one ordered scale: CEFR A1..C2 = 1..6, native/bilingual = 7.
pub(crate) type Level = u8;
pub(super) const FLUENT: Level = 5;
const NATIVE: Level = 7;

/// `(canonical name, words that name it)`; words are matched whole, lowercased.
const LANGUAGES: &[(&str, &[&str])] = &[
    (
        "English",
        &[
            "english", "englisch", "inglese", "anglais", "inglés", "ingles",
        ],
    ),
    (
        "Italian",
        &[
            "italian",
            "italiano",
            "italiana",
            "italienisch",
            "italien",
            "italiane",
        ],
    ),
    (
        "German",
        &[
            "german", "deutsch", "tedesco", "allemand", "alemán", "aleman",
        ],
    ),
    (
        "French",
        &["french", "französisch", "francese", "français", "francais"],
    ),
    (
        "Spanish",
        &["spanish", "spanisch", "spagnolo", "español", "espanol"],
    ),
    (
        "Portuguese",
        &["portuguese", "portugiesisch", "portoghese", "português"],
    ),
    (
        "Dutch",
        &["dutch", "niederländisch", "olandese", "nederlands"],
    ),
];

/// Level words → rank. CEFR codes are matched separately.
fn word_level(word: &str, prev: Option<&str>) -> Option<Level> {
    Some(match word {
        "a1" => 1,
        "a2" => 2,
        "b1" => 3,
        "b2" => 4,
        "c1" => 5,
        "c2" => 6,
        "basic" | "elementary" | "beginner" | "grundkenntnisse" | "basilare" | "scolastico" => 2,
        "conversational" | "intermediate" | "mittelstufe" | "intermedio" => 3,
        "advanced" | "proficient" | "fluent" | "fluente" | "fließend" | "fliessend" | "courant"
        | "avanzato" | "fortgeschritten" => FLUENT,
        "professional" if prev == Some("full") => FLUENT,
        "native" | "madrelingua" | "muttersprache" | "muttersprachlich" | "muttersprachler"
        | "nativo" | "bilingual" | "bilingue" | "mother" => NATIVE,
        _ => return None,
    })
}

fn words(segment: &str) -> Vec<String> {
    segment
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Segments that each carry at most one stated level: split on list/clause punctuation and on
/// "and"-style conjunctions so "Italian B1 and English C2" does not smear one level over both.
fn segments(text: &str) -> Vec<String> {
    let mut out = vec![text.to_lowercase()];
    for sep in [
        "\n", ",", ";", "|", "•", "·", "/", " and ", " und ", " e ", " et ", " y ", " but ",
    ] {
        out = out
            .iter()
            .flat_map(|s| s.split(sep).map(str::to_string).collect::<Vec<_>>())
            .collect();
    }
    out
}

/// Split at sentence ends (`.`/`!`/`?` followed by whitespace or the end) so a level never bleeds
/// from one sentence into the next ("Italian B1. English C2.").
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|(_, n)| n.is_whitespace()) {
            out.push(&text[start..i]);
            start = i + c.len_utf8();
        }
    }
    out.push(&text[start..]);
    out
}

const NEGATORS: &[&str] = &[
    "not", "no", "non", "nicht", "kein", "keine", "never", "without",
];

/// A level word right after a negator ("not fluent", "nicht fließend") asserts nothing.
fn negated(ws: &[String], i: usize) -> bool {
    (i.saturating_sub(2)..i).any(|p| NEGATORS.contains(&ws[p].as_str()))
}

/// Every `(language, level)` the text asserts. A segment with a level applies it to each language
/// in it; a level that PRECEDES its language ("fluent in English and Italian") is shared with the
/// following level-less segments of the same sentence, a trailing one ("English C2") is not.
pub(crate) fn language_levels(text: &str) -> Vec<(&'static str, Level)> {
    let mut found = Vec::new();
    for sentence in sentences(text) {
        let mut carry: Option<Level> = None;
        for seg in segments(sentence) {
            let ws = words(&seg);
            let levels: Vec<(usize, Level)> = (0..ws.len())
                .filter(|&i| !negated(&ws, i))
                .filter_map(|i| {
                    let prev = i.checked_sub(1).map(|p| ws[p].as_str());
                    word_level(&ws[i], prev).map(|l| (i, l))
                })
                .collect();
            let langs: Vec<(&'static str, usize)> = LANGUAGES
                .iter()
                .filter_map(|(name, aliases)| {
                    let at = ws.iter().position(|w| aliases.contains(&w.as_str()))?;
                    Some((*name, at))
                })
                .collect();
            let own = levels.iter().map(|(_, l)| *l).max();
            let level = own.or(carry);
            if let Some(own) = own {
                let first_level = levels.iter().map(|(i, _)| *i).min();
                let first_lang = langs.iter().map(|(_, at)| *at).min();
                carry = first_level
                    .zip(first_lang)
                    .and_then(|(lv, lg)| (lv < lg).then_some(own));
            } else if langs.is_empty() {
                carry = None;
            }
            if let Some(level) = level {
                found.extend(langs.iter().map(|(name, _)| (*name, level)));
            }
        }
    }
    found
}

/// The best level the résumé evidences per language. Absent = no evidence (unknown).
pub(crate) fn evidenced_levels(resume: &str) -> HashMap<&'static str, Level> {
    let mut map: HashMap<&'static str, Level> = HashMap::new();
    for (lang, level) in language_levels(resume) {
        let e = map.entry(lang).or_insert(0);
        *e = (*e).max(level);
    }
    map
}
