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

/// Every `(language, level)` the text asserts. A segment with a level and several languages
/// ("fluent English and Italian" split apart aside) applies the level to each of them.
pub(crate) fn language_levels(text: &str) -> Vec<(&'static str, Level)> {
    let mut found = Vec::new();
    for seg in segments(text) {
        let ws = words(&seg);
        let level = ws
            .iter()
            .enumerate()
            .filter_map(|(i, w)| word_level(w, i.checked_sub(1).map(|p| ws[p].as_str())))
            .max();
        let Some(level) = level else { continue };
        for (name, aliases) in LANGUAGES {
            if ws.iter().any(|w| aliases.contains(&w.as_str())) {
                found.push((*name, level));
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
