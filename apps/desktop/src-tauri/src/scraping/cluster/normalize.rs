//! Pure company/title normalization for cross-board clustering (ADR-029 §d).
//!
//! Deterministic, allocation-light, and dependency-free beyond the existing
//! `regex` crate — NO `strsim`/`unicode-normalization` (ADR-029: ~stdlib lines
//! suffice, blocking does the discriminative work). Every function here is a
//! pure `&str -> String`/`bool`, so the whole surface is unit-testable without a
//! store, a runtime, or the network.
//!
//! The normalized forms are used two ways in [`super`]: the blocking key
//! (`normalize_company` + [`title_first_token`]) and the string-path join test
//! (trigram-Jaccard over `normalize_title`). They are NEVER shown to the user —
//! the renderer echoes opaque `clusterId`/member keys only (ADR-029 §e), so
//! there is no TypeScript mirror of this logic.

use std::sync::LazyLock;

use regex::Regex;

// ── Folding ─────────────────────────────────────────────────────────────────

/// Case- and diacritic-fold a string for comparison.
///
/// 1. lowercase, then
/// 2. expand the German umlauts/eszett to their ASCII digraphs
///    (`ä→ae`, `ö→oe`, `ü→ue`, `ß→ss`) so `Müller` ≡ `Mueller`, then
/// 3. map ~30 other precomposed Latin diacritics to their base letter
///    (`é→e`, `ñ→n`, `ç→c`, …), and
/// 4. drop Unicode combining marks (U+0300–U+036F) so a *decomposed* accent
///    (`e` + U+0301) also folds to its base letter.
///
/// German expansion runs BEFORE the generic base-letter map so `ä` becomes `ae`,
/// not `a` — the digraph is the discriminative German convention.
pub fn fold(s: &str) -> String {
    let lowered = s.to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    for ch in lowered.chars() {
        match ch {
            'ä' => out.push_str("ae"),
            'ö' => out.push_str("oe"),
            'ü' => out.push_str("ue"),
            'ß' => out.push_str("ss"),
            // Combining diacritical marks — drop, leaving the base char intact.
            c if ('\u{0300}'..='\u{036f}').contains(&c) => {}
            c => out.push(fold_diacritic(c)),
        }
    }
    out
}

/// Map a single precomposed Latin-diacritic char to its base letter, or return
/// it unchanged. `ä`/`ö`/`ü`/`ß` are handled by [`fold`] (digraph expansion)
/// before this is reached, so they are intentionally absent here.
fn fold_diacritic(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ù' | 'ú' | 'û' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ź' | 'ż' | 'ž' => 'z',
        'ý' | 'ÿ' => 'y',
        'ŕ' | 'ř' => 'r',
        'ł' | 'ĺ' | 'ļ' | 'ľ' => 'l',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ţ' | 'ť' => 't',
        'ď' | 'đ' => 'd',
        other => other,
    }
}

// ── Company ─────────────────────────────────────────────────────────────────

/// Trailing legal-form suffixes to strip, LONGEST-FIRST so a compound form
/// (`gmbh & co. kg`) is consumed whole before its shorter tail (`kg`) can
/// pre-empt it. Whole trailing tokens only (see [`strip_legal_suffixes`]).
/// Closed set (ADR-029 §i) — matched against the FOLDED, still-punctuated string.
const LEGAL_SUFFIXES: &[&str] = &[
    "gmbh & co. kg",
    "gmbh & co kg",
    "e.v.",
    "gmbh",
    "inc",
    "llc",
    "ltd",
    "ag",
    "se",
    "kg",
    "ev",
    "co",
];

/// Normalize a company name to its blocking form: [`fold`] → strip trailing
/// legal suffixes (iteratively, longest-first) → collapse punctuation/whitespace
/// runs to single spaces → trim. `"Acme GmbH & Co. KG"` → `"acme"`.
pub fn normalize_company(s: &str) -> String {
    let folded = fold(s);
    let stripped = strip_legal_suffixes(folded);
    collapse_punct(&stripped)
}

/// Iteratively strip trailing legal suffixes. A suffix is only stripped when it
/// sits on a token boundary — the char immediately before the match must be a
/// non-alphanumeric (or the match is the whole remaining string) — so `co` peels
/// `"acme co"` → `"acme"` but never `"cisco"` → `"cis"`. Trailing punctuation is
/// trimmed before each attempt, so `"acme gmbh."` is handled.
fn strip_legal_suffixes(mut s: String) -> String {
    loop {
        // Only lengths (usize) escape the borrows of `s`, so the truncation below
        // never conflicts with the immutable slices. Match each suffix against
        // BOTH the whitespace-trimmed form (so a suffix that itself ends in
        // punctuation, `e.v.`, matches) AND the punctuation-trimmed form (so a
        // suffix followed by stray punctuation, `GmbH.`, matches too).
        let ws_trimmed = s.trim_end();
        let punct_trimmed = ws_trimmed.trim_end_matches(|c: char| !c.is_alphanumeric());
        let punct_len = punct_trimmed.len();
        let mut keep_len: Option<usize> = None;
        for suffix in LEGAL_SUFFIXES {
            let prefix = ws_trimmed
                .strip_suffix(suffix)
                .or_else(|| punct_trimmed.strip_suffix(suffix));
            if let Some(prefix) = prefix {
                let on_boundary = prefix
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_alphanumeric());
                if on_boundary {
                    keep_len = Some(prefix.len());
                    break;
                }
            }
        }
        match keep_len {
            Some(len) => s.truncate(len),
            None => {
                // Persist the trailing-punctuation trim into the returned value.
                s.truncate(punct_len);
                return s;
            }
        }
    }
}

/// Replace every run of non-alphanumeric chars (punctuation + whitespace) with a
/// single space and trim — the aggressive company collapse. `"acme,  inc."` (if
/// `inc` weren't a suffix) → `"acme inc"`.
fn collapse_punct(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Agency detection (ADR-029 §i) ───────────────────────────────────────────

/// Built-in staffing/recruiting agencies, stored in normalized form so a match
/// is a straight equality against [`normalize_company`]'s output.
const AGENCY_COMPANIES: &[&str] = &[
    "hays",
    "michael page",
    "randstad",
    "adecco",
    "robert half",
    "academic work",
];

/// Whole-word agency signal tokens (German + English). A company whose
/// normalized words contain any of these reads as an agency.
const AGENCY_TOKENS: &[&str] = &["personalberatung", "recruiting", "staffing"];

/// Whether `company` is a recruiting/staffing agency: its normalized form equals
/// a built-in (or a user-supplied `extra`, normalized the SAME way) agency name,
/// or one of its whole words is an agency signal token. Extras let a user flag
/// their region's agencies without a code change (ADR-029 §i).
pub fn is_agency(company: &str, extra: &[String]) -> bool {
    let norm = normalize_company(company);
    if norm.is_empty() {
        return false;
    }
    if AGENCY_COMPANIES.contains(&norm.as_str()) {
        return true;
    }
    if extra
        .iter()
        .any(|e| !e.trim().is_empty() && normalize_company(e) == norm)
    {
        return true;
    }
    norm.split_whitespace().any(|w| AGENCY_TOKENS.contains(&w))
}

// ── Title ───────────────────────────────────────────────────────────────────

/// Gender tags that may appear inside a `(m/w/d)`-family parenthetical or as a
/// bare trailing sequence. `all genders` is handled as a phrase separately.
const GENDER_TOKENS: &[&str] = &["m", "w", "d", "x", "f", "h", "divers", "gn"];

/// Seniority/level words a trailing segment must NOT be stripped over — dropping
/// a "senior"/"junior"/… qualifier would merge genuinely distinct roles.
const PROTECTED_TITLE_TOKENS: &[&str] =
    &["senior", "junior", "lead", "principal", "staff", "intern"];

/// Remote/on-site markers that qualify a trailing dash/pipe or parenthetical
/// segment as a location/work-mode suffix to strip. Folded ASCII forms.
const REMOTE_MARKERS: &[&str] = &[
    "remote",
    "hybrid",
    "home office",
    "homeoffice",
    "home-office",
    "on-site",
    "onsite",
    "vor ort",
    "work from home",
    "wfh",
];

/// A parenthetical group `(...)`; the whole group is removed when its content is
/// purely gender tags (see [`is_gender_content`]).
static PAREN_GROUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\([^)]*\)").expect("valid"));

/// A bare trailing gender sequence — at least two `/|,`-separated gender tags at
/// the end of the string, optionally preceded by dash/pipe/comma/space — e.g.
/// `"… developer m/w/d"` or `"… developer - m/w/d"`. Requires ≥2 tags so a real
/// trailing single word is never mistaken for a gender tag.
static BARE_GENDER_TAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        [\s/|,\-–—]+
        (?:m|w|d|x|f|h|divers|gn)
        (?:\s*[/|,]\s*(?:m|w|d|x|f|h|divers|gn))+
        \s*$",
    )
    .expect("valid")
});

/// Normalize a job title for the string-path join test: [`fold`] → remove gender
/// parentheticals + a bare trailing gender sequence → strip a trailing
/// location/remote segment (never one carrying a seniority word) → collapse
/// whitespace runs. `"Senior Rust Developer (m/w/d) – Berlin"` →
/// `"senior rust developer"`.
pub fn normalize_title(s: &str) -> String {
    let folded = fold(s);
    let no_gender_parens = PAREN_GROUP.replace_all(&folded, |caps: &regex::Captures<'_>| {
        let whole = &caps[0];
        let inner = &whole[1..whole.len() - 1];
        if is_gender_content(inner) {
            " ".to_string()
        } else {
            whole.to_string()
        }
    });
    let no_bare_gender = BARE_GENDER_TAIL.replace(&no_gender_parens, "");
    let no_locrem = strip_trailing_locrem(no_bare_gender.into_owned());
    collapse_ws(&no_locrem)
}

/// The first whitespace-delimited token of the normalized title — half of the
/// blocking key, so `Senior …` and `Junior …` never share a block (ADR-029 §c).
pub fn title_first_token(title: &str) -> String {
    normalize_title(title)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

/// Whether a parenthetical's inner content is ONLY gender tags — either the
/// `all genders` phrase, or `/|,`-separated tags all drawn from [`GENDER_TOKENS`].
fn is_gender_content(inner: &str) -> bool {
    let inner = inner.trim();
    if inner == "all genders" {
        return true;
    }
    let parts: Vec<&str> = inner
        .split(['/', '|', ','])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    !parts.is_empty() && parts.iter().all(|p| GENDER_TOKENS.contains(p))
}

/// Iteratively strip a trailing location/remote segment — either a parenthetical
/// `(...)` or a `-`/`–`/`—`/`|`-delimited tail — while it reads as location/remote
/// (see [`is_locrem_segment`]). Never strips a segment carrying a seniority word.
///
/// A trailing PARENTHETICAL is only stripped on an explicit remote/on-site
/// marker (`(Remote)`), never on a bare word — `(Backend)`/`(Java)` are role
/// qualifiers, not locations, so keeping them avoids merging distinct roles. A
/// DASH/PIPE tail additionally strips a single all-alphabetic token as a bare
/// place name (`– Berlin`), the common "title – city" convention. Documented
/// residual: a single-word tech qualifier after a dash (`Engineer - Java`) is
/// also dropped — an accepted recall-favoring tradeoff for same-company,
/// same-first-token clustering (ADR-029 §c).
fn strip_trailing_locrem(mut s: String) -> String {
    loop {
        let t = s.trim_end();
        // Trailing parenthetical — remote-marker only (no bare-place stripping).
        if t.ends_with(')') {
            if let Some(open) = t.rfind('(') {
                let inner = &t[open + 1..t.len() - 1];
                if is_locrem_segment(inner, false) {
                    s = t[..open].to_string();
                    continue;
                }
            }
        }
        // Trailing dash/pipe segment — remote marker OR a bare place name.
        if let Some(pos) = t.rfind(['|', '-', '–', '—']) {
            let delim_len = t[pos..].chars().next().map_or(1, char::len_utf8);
            let seg = &t[pos + delim_len..];
            if is_locrem_segment(seg, true) {
                s = t[..pos].to_string();
                continue;
            }
        }
        return t.to_string();
    }
}

/// Whether a trailing title segment reads as a location/work-mode suffix worth
/// stripping: it must NOT contain a seniority word, and it must either carry a
/// remote/on-site marker OR (when `allow_bare_place`) be a single all-alphabetic
/// token. The single-token bar keeps a multi-word qualifier (`machine learning`)
/// from being mistaken for a location.
fn is_locrem_segment(seg: &str, allow_bare_place: bool) -> bool {
    let seg = seg.trim();
    if seg.is_empty() {
        return false;
    }
    let words: Vec<&str> = seg.split_whitespace().collect();
    if words
        .iter()
        .any(|w| PROTECTED_TITLE_TOKENS.contains(&w.trim_matches(|c: char| !c.is_alphanumeric())))
    {
        return false;
    }
    if REMOTE_MARKERS.iter().any(|m| seg.contains(m)) {
        return true;
    }
    allow_bare_place && words.len() == 1 && words[0].chars().all(char::is_alphabetic)
}

/// Collapse whitespace runs to single spaces and trim — the gentle title
/// collapse that preserves in-word punctuation (`c++`, `node.js`) for trigrams.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── fold ────────────────────────────────────────────────────────────────

    #[test]
    fn fold_lowercases_and_expands_german_umlauts() {
        // Precomposed umlaut → ASCII digraph, so Müller ≡ Mueller.
        assert_eq!(fold("Müller"), "mueller");
        assert_eq!(fold("Mueller"), "mueller");
        assert_eq!(fold("Größe"), "groesse"); // ö→oe AND ß→ss
        assert_eq!(fold("Über"), "ueber");
    }

    #[test]
    fn fold_strips_combining_marks_to_base_letter() {
        // Decomposed acute (e + U+0301) → base letter, no accent left behind.
        assert_eq!(fold("Cafe\u{0301}"), "cafe");
        // Precomposed accents map to their base letter too.
        assert_eq!(fold("Peña"), "pena");
        assert_eq!(fold("Škoda"), "skoda");
    }

    // ── normalize_company ─────────────────────────────────────────────────────

    #[test]
    fn company_strips_compound_legal_suffix() {
        assert_eq!(normalize_company("Acme GmbH & Co. KG"), "acme");
        assert_eq!(normalize_company("Acme GmbH"), "acme");
        assert_eq!(normalize_company("Acme, Inc."), "acme");
        assert_eq!(normalize_company("Beispiel Verein e.V."), "beispiel verein");
    }

    #[test]
    fn company_strips_stacked_suffixes_iteratively() {
        // "ag" then "gmbh" both peel off, longest tail first.
        assert_eq!(normalize_company("Foo GmbH AG"), "foo");
    }

    #[test]
    fn company_keeps_suffix_that_is_not_a_whole_token() {
        // "co" is a suffix, but "cisco" must not lose its tail.
        assert_eq!(normalize_company("Cisco"), "cisco");
        // "adecco" ends in "co" mid-token — kept whole.
        assert_eq!(normalize_company("Adecco"), "adecco");
    }

    // ── is_agency ─────────────────────────────────────────────────────────────

    #[test]
    fn agency_matches_builtin_names_tokens_and_extras() {
        // Built-in company names.
        assert!(is_agency("Hays", &[]));
        assert!(is_agency("Michael Page", &[]));
        assert!(is_agency("Randstad", &[]));
        // Token signal (German + English), even with a legal suffix present.
        assert!(is_agency("Mustermann Personalberatung GmbH", &[]));
        assert!(is_agency("Acme Recruiting", &[]));
        // User-supplied extra, normalized the same way.
        assert!(is_agency("Talent Partners AG", &["talent partners".to_string()]));
        // A real employer is not an agency.
        assert!(!is_agency("Acme", &[]));
        assert!(!is_agency("", &[]));
    }

    // ── normalize_title: gender tags ─────────────────────────────────────────

    #[test]
    fn title_strips_all_gender_tag_variants() {
        for variant in [
            "Rust Developer (m/w/d)",
            "Rust Developer (w/m/d)",
            "Rust Developer (m/w/x)",
            "Rust Developer (d/m/w)",
            "Rust Developer (all genders)",
            "Rust Developer (gn)",
            "Rust Developer m/w/d",
        ] {
            assert_eq!(
                normalize_title(variant),
                "rust developer",
                "variant `{variant}` must fold to the bare title"
            );
        }
    }

    #[test]
    fn title_keeps_a_role_qualifier_parenthetical() {
        // A parenthetical role qualifier is NOT a gender tag and NOT a remote
        // marker, so it is preserved — dropping it would merge distinct roles
        // ("(Backend)" vs "(Frontend)") at the same company.
        assert_eq!(normalize_title("Developer (Backend)"), "developer (backend)");
        // But an explicit remote-marker parenthetical is stripped.
        assert_eq!(normalize_title("Developer (Remote)"), "developer");
    }

    // ── normalize_title: seniority + location ────────────────────────────────

    #[test]
    fn title_keeps_seniority_words() {
        assert_eq!(normalize_title("Senior Rust Developer"), "senior rust developer");
        assert_eq!(normalize_title("Junior Rust Developer"), "junior rust developer");
        // A seniority word in a trailing segment is never stripped.
        assert_eq!(
            normalize_title("Rust Developer - Senior Team"),
            "rust developer - senior team"
        );
    }

    #[test]
    fn title_strips_trailing_location_and_remote() {
        assert_eq!(
            normalize_title("Senior Rust Developer (m/w/d) – Berlin"),
            "senior rust developer"
        );
        assert_eq!(normalize_title("Backend Engineer | Remote"), "backend engineer");
        assert_eq!(normalize_title("Data Engineer (Remote)"), "data engineer");
        assert_eq!(normalize_title("Platform Engineer - Home Office"), "platform engineer");
    }

    #[test]
    fn title_first_token_drives_the_block() {
        assert_eq!(title_first_token("Senior Rust Developer (m/w/d)"), "senior");
        assert_eq!(title_first_token("Junior Rust Developer"), "junior");
        assert_eq!(title_first_token(""), "");
    }
}
