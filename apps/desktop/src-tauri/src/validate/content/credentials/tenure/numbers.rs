//! Reading a `<number> <year-word>` span: the number words every language the
//! pipeline writes, the year words, and the regexes built from them.

use std::sync::LazyLock;

use regex::Regex;

use super::MAX_PLAUSIBLE_TENURE_YEARS;

/// Number words a tenure is spelled with, in EVERY language `make_stemmer`
/// supports — not just the two whose lexicons this module curates.
///
/// Digits are handled by the regex; this table exists because a truthful résumé
/// spells its tenure out ("eight years", "acht Jahre", "quinze ans"), and the
/// side that reads the SOURCE is the side that SPARES a claim. An en/de-only
/// table therefore did not merely miss French — it read `quinze années
/// d'expérience` as a source that states nothing, and turned a faithful
/// `15 années` output into a Critical. That is the whole "translation-safe by
/// construction" argument failing at the one place it mattered: the comparison
/// is on a number, but the EXTRACTION of the sparing evidence was not.
///
/// Duplicates across languages ("six" en/fr, "vier" de/nl, "acht" de/nl) are
/// harmless and deliberate — every spelling maps to the same value, and the
/// regex alternation is deduplicated where it is built.
pub(super) const SPELLED_NUMBERS: &[(&str, u32)] = &[
    // English
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
    ("ein", 1),
    ("eins", 1),
    ("zwei", 2),
    ("drei", 3),
    ("vier", 4),
    ("fünf", 5),
    ("sechs", 6),
    ("sieben", 7),
    ("acht", 8),
    ("neun", 9),
    ("zehn", 10),
    ("elf", 11),
    ("zwölf", 12),
    ("dreizehn", 13),
    ("vierzehn", 14),
    ("fünfzehn", 15),
    ("sechzehn", 16),
    ("siebzehn", 17),
    ("achtzehn", 18),
    ("neunzehn", 19),
    ("zwanzig", 20),
    // French
    ("un", 1),
    ("une", 1),
    ("deux", 2),
    ("trois", 3),
    ("quatre", 4),
    ("cinq", 5),
    ("sept", 7),
    ("huit", 8),
    ("neuf", 9),
    ("dix", 10),
    ("onze", 11),
    ("douze", 12),
    ("treize", 13),
    ("quatorze", 14),
    ("quinze", 15),
    ("seize", 16),
    ("dix-sept", 17),
    ("dix-huit", 18),
    ("dix-neuf", 19),
    ("vingt", 20),
    // Spanish
    ("uno", 1),
    ("una", 1),
    ("dos", 2),
    ("tres", 3),
    ("cuatro", 4),
    ("cinco", 5),
    ("seis", 6),
    ("siete", 7),
    ("ocho", 8),
    ("nueve", 9),
    ("diez", 10),
    ("once", 11),
    ("doce", 12),
    ("trece", 13),
    ("catorce", 14),
    ("quince", 15),
    ("dieciséis", 16),
    ("dieciseis", 16),
    ("diecisiete", 17),
    ("dieciocho", 18),
    ("diecinueve", 19),
    ("veinte", 20),
    // Italian
    ("due", 2),
    ("tre", 3),
    ("quattro", 4),
    ("cinque", 5),
    ("sei", 6),
    ("sette", 7),
    ("otto", 8),
    ("nove", 9),
    ("dieci", 10),
    ("undici", 11),
    ("dodici", 12),
    ("tredici", 13),
    ("quattordici", 14),
    ("quindici", 15),
    ("sedici", 16),
    ("diciassette", 17),
    ("diciotto", 18),
    ("diciannove", 19),
    ("venti", 20),
    // Dutch
    ("een", 1),
    ("één", 1),
    ("twee", 2),
    ("drie", 3),
    ("vijf", 5),
    ("zes", 6),
    ("zeven", 7),
    ("negen", 9),
    ("tien", 10),
    ("twaalf", 12),
    ("dertien", 13),
    ("veertien", 14),
    ("vijftien", 15),
    ("zestien", 16),
    ("zeventien", 17),
    ("achttien", 18),
    ("negentien", 19),
    ("twintig", 20),
    // Portuguese
    ("um", 1),
    ("uma", 1),
    ("dois", 2),
    ("duas", 2),
    ("três", 3),
    ("tres", 3),
    ("sete", 7),
    ("oito", 8),
    ("dez", 10),
    ("doze", 12),
    ("treze", 13),
    ("catorze", 14),
    ("quatorze", 14),
    ("dezesseis", 16),
    ("dezessete", 17),
    ("dezoito", 18),
    ("dezenove", 19),
    ("vinte", 20),
];

/// The word for "year" in the languages `make_stemmer` supports.
///
/// Bare French `an` is deliberately absent: it is one letter away from the
/// English article in every document this runs on, and a singular tenure is
/// not worth that.
const YEAR_WORDS: &str = "years|year|yrs|yr|jahren|jahre|jahr|années|année|annees|annee|ans|años|año|anos|ano|anni|anno|jaren|jaar";

/// The distinct spellings [`YEARS_RE`] alternates on, longest first — `\b`
/// alone does not decide an alternation, and `vier` would otherwise win
/// against `vierzehn`.
///
/// Deduplicated by VALUE first, not by adjacency after the length sort:
/// `Vec::dedup` removes only CONSECUTIVE equal elements, and sorting by
/// LENGTH groups equal lengths, not equal strings — a few spellings repeat
/// across language blocks at a different position in the table (`quatorze` is
/// French 14 and repeats in the Portuguese block; `tres` is Spanish 3 and
/// repeats in the Portuguese block), and other same-length words sit between
/// the two copies, so a length-only sort never makes them adjacent.
pub fn spelled_number_words() -> Vec<&'static str> {
    let mut words: Vec<&str> = SPELLED_NUMBERS.iter().map(|(w, _)| *w).collect();
    words.sort_unstable();
    words.dedup();
    words.sort_by_key(|w| std::cmp::Reverse(w.len()));
    words
}

/// `<number> <year-word>`, where the separator is whitespace or a `+`.
///
/// The separator rule is what keeps the hyphenated ADJECTIVE out: "a 20-year-old
/// legacy stack" is not a claim about anybody's tenure, and it is the single
/// most common two-digit `year` collocation in engineering prose. `\b` on the
/// number keeps the check out of a four-digit date ("2014 - 2018" offers no
/// position where one or two digits end on a word boundary).
static YEARS_RE: LazyLock<Regex> = LazyLock::new(|| {
    let words = spelled_number_words();
    Regex::new(&format!(
        r"(?i)\b(\d{{1,2}}|{})(?:\s*\+\s*|\s+)({})\b",
        words.join("|"),
        YEAR_WORDS
    ))
    .unwrap()
});

/// A year-word preceded by a WORD rather than a digit — the shape
/// [`states_an_unreadable_tenure`] inspects.
///
/// Deliberately looser than [`YEARS_RE`]: this one has to see the quantifiers
/// that table CANNOT read, which is the whole point of it.
pub(super) static UNREADABLE_TENURE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?i)\b(\p{{L}}[\p{{L}}-]*)\s+({YEAR_WORDS})\b")).unwrap()
});

/// The number a `YEARS_RE` capture states, in digits or in words.
fn claimed_years(raw: &str) -> Option<u32> {
    if let Ok(n) = raw.parse::<u32>() {
        return Some(n);
    }
    let lower = raw.to_lowercase();
    SPELLED_NUMBERS
        .iter()
        .find(|(word, _)| *word == lower)
        .map(|(_, n)| *n)
}

/// Every `<number> years` span on `line`, unfiltered — the SOURCE side's
/// reading, which needs no context word.
pub(super) fn years_spans(line: &str) -> Vec<(u32, String, usize, usize)> {
    YEARS_RE
        .captures_iter(line)
        .filter_map(|c| {
            let whole = c.get(0)?;
            let years = claimed_years(c.get(1)?.as_str())?;
            (years <= MAX_PLAUSIBLE_TENURE_YEARS).then(|| {
                (
                    years,
                    whole.as_str().trim().to_string(),
                    whole.start(),
                    whole.end(),
                )
            })
        })
        .collect()
}
