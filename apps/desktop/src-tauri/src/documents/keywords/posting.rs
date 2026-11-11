//! Turning a job posting into the text the keyword kernel scores: the blank-description test, the
//! markdown stripper and the title + description + requirements blob.
//!
//! Split out of `documents/keywords.rs` (R8's hard LOC cap); everything here moved verbatim.

/// Known LinkedIn guest-wall / footer / navigation strings (lowercase). A description made only of
/// these (plus URLs and punctuation) is not a job ad (issue #1443). Phrases are deliberately
/// multi-word so real ad prose is not eaten; stripping only matters when NOTHING else remains.
const BOILERPLATE_PHRASES: &[&str] = &[
    "sign in to view more",
    "sign in to see more",
    "sign in to apply",
    "sign in",
    "join now",
    "join linkedin",
    "new to linkedin",
    "continue with google",
    "welcome back",
    "forgot password",
    "show more",
    "show less",
    "see more",
    "see less",
    "similar jobs",
    "people also viewed",
    "set job alert",
    "user agreement",
    "privacy policy",
    "cookie policy",
    "copyright policy",
    "your california privacy choices",
    "brand policy",
    "guest controls",
    "community guidelines",
    "linkedin corporation",
    "accept & join",
    "agree & join",
    "by clicking continue",
    "skip to main content",
    "accedi per visualizzare",
    "iscriviti ora",
    "mostra altro",
    "mostra meno",
    "offerte simili",
    "anmelden",
    "jetzt anmelden",
    "mehr anzeigen",
    "weniger anzeigen",
    "ähnliche jobs",
    "se connecter",
    "rejoindre maintenant",
    "afficher plus",
    "afficher moins",
    "offres similaires",
    "iniciar sesión",
    "unirse ahora",
    "ver más",
    "ver menos",
    "empleos similares",
];

/// Whether a job description carries no usable ad content: absent, empty once markdown syntax is
/// stripped ([`markdown_to_plain`]), or only LinkedIn login-wall / footer / navigation
/// boilerplate ([`BOILERPLATE_PHRASES`]) with no real text left. Length alone never makes a
/// description blank, so a short genuine ad or a multilingual one stays non-blank.
/// The description-only half of `commands::autopilot::build_found_job`'s `no_jd_text` predicate,
/// shared with `AutopilotStore::update_found_job_descriptions` and LinkedIn enrichment so they
/// can't drift on what counts as "no text" (issues #1106, #1443).
pub fn description_is_blank(description: Option<&str>) -> bool {
    let Some(d) = description else { return true };
    !markdown_to_plain(d).lines().any(|line| {
        let l = line.trim().to_lowercase();
        let bare = l.trim_matches(|c: char| !c.is_alphanumeric());
        if WHOLE_LINE_TOKENS.contains(&bare) || is_language_picker_line(line.trim()) {
            return false;
        }
        let mut rest = l;
        for p in BOILERPLATE_PHRASES {
            rest = rest.replace(p, " ");
        }
        rest.chars().any(char::is_alphabetic)
    })
}

/// Single words that are footer chrome only when they are a whole line by themselves; stripping
/// them from inside a sentence ("Tell us about yourself") would corrupt real ad text.
const WHOLE_LINE_TOKENS: &[&str] = &["about", "accessibility", "language", "select language"];

/// LinkedIn guest language-picker line: short `<native name> (<Capitalized English name>)`,
/// e.g. `Deutsch (German)`, `العربية (Arabic)`.
fn is_language_picker_line(line: &str) -> bool {
    let Some(inner) = line.strip_suffix(')').and_then(|l| l.rsplit_once(" (")) else {
        return false;
    };
    let (outer, name) = inner;
    let words = |t: &str| t.split_whitespace().count();
    (1..=3).contains(&words(outer))
        && (1..=2).contains(&words(name))
        && name.split_whitespace().all(|w| {
            let mut c = w.chars();
            c.next().is_some_and(char::is_uppercase) && c.all(|x| x.is_alphabetic() || x == '-')
        })
}

/// Strip markdown syntax from a scoring text blob so URL fragments and
/// formatting tokens do not pollute the ATS keyword set.
///
/// Applied to the `description` field of `posting_text_blob` ONLY — the stored
/// `description` is not touched (the frontend renders that markdown).
///
/// Rules (applied in order):
/// 1. Inline links `[anchor](url)` → anchor text only (restores old HTML-strip
///    behaviour: href dropped, visible text kept).
/// 2. Bare URLs (`https?://…`) → removed (no visible text to keep).
/// 3. Heading markers (`# …`) → leading `#` characters stripped.
/// 4. `*` emphasis markers → removed. `_` is deliberately kept: underscores
///    are part of real tech tokens (`OPENAI_API_KEY`, `next_js`) and stripping
///    them blanket-corrupts ATS keyword extraction. Markdown `_` emphasis is
///    rare in scraped JD text and does not need dedicated removal here.
pub fn markdown_to_plain(text: &str) -> String {
    // A stored description that arrived entity-escaped (`&lt;p&gt;`, #1497) is decoded
    // here, at the point of use, so scoring sees words rather than markup residue.
    let converted;
    let text = if crate::scraping::http::has_escaped_markup(text) {
        converted = crate::scraping::http::html_to_markdown(text);
        converted.as_str()
    } else {
        text
    };
    // Step 1: collapse `[anchor text](url)` → `anchor text`.
    // Operates entirely on &str slices (char-boundary-safe) — never byte as char.
    let no_links = {
        let mut out = String::with_capacity(text.len());
        let mut remaining = text;
        while let Some(open) = remaining.find('[') {
            // Emit the text before the `[`.
            out.push_str(&remaining[..open]);
            let after_open = &remaining[open + 1..]; // char after `[`
                                                     // Look for `](` that closes this link's anchor.
            if let Some(close_bracket) = after_open.find("](") {
                let anchor = &after_open[..close_bracket];
                let after_bracket = &after_open[close_bracket + 2..]; // past `](`
                if let Some(close_paren) = after_bracket.find(')') {
                    // Valid `[anchor](url)` — emit only the anchor text.
                    out.push_str(anchor);
                    remaining = &after_bracket[close_paren + 1..];
                    continue;
                }
            }
            // Not a valid link syntax — emit the `[` literally and advance past it.
            out.push('[');
            remaining = after_open;
        }
        // Emit whatever is left after the last `[` (or the whole string if none).
        out.push_str(remaining);
        out
    };

    // Step 2: remove bare URLs (`https?://` followed by non-whitespace chars).
    let no_urls = {
        let mut out = String::with_capacity(no_links.len());
        let mut remaining = no_links.as_str();
        while let Some(pos) = remaining.find("http") {
            let prefix = &remaining[..pos];
            out.push_str(prefix);
            let tail = &remaining[pos..];
            if tail.starts_with("https://") || tail.starts_with("http://") {
                // Skip all non-whitespace chars of the URL.
                let url_len = tail.find(|c: char| c.is_whitespace()).unwrap_or(tail.len());
                remaining = &tail[url_len..];
            } else {
                // "http" but not a full URL prefix — emit the char and advance.
                out.push('h');
                remaining = &tail[1..];
            }
        }
        out.push_str(remaining);
        out
    };

    // Step 3: strip leading heading markers (`# `, `## `, etc.) per line.
    // Step 4: remove `*` emphasis markers only — do NOT strip `_`.
    //
    // Underscores are part of real tech tokens (OPENAI_API_KEY, next_js,
    // MY_ENV_VAR). Stripping every `_` blanket-corrupts those tokens before
    // keyword extraction, breaking ATS matching. `_` as a markdown emphasis
    // delimiter is rare in scraped JD text, and even when present the
    // tokenizer in `keywords_normalized` already splits on non-alphanumeric
    // characters (excluding `_` is not needed there). Keep `_` intact.
    no_urls
        .lines()
        .map(|line| {
            let trimmed = line.trim_start_matches('#').trim_start();
            trimmed.replace('*', "")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the ATS text blob for a job posting — title + description + requirements,
/// joined by newlines. Single source of truth shared by the Jobs-page scorer
/// (`commands::match_resume`) and the headless Autopilot ranker, so both score
/// identical text. Returns None when there's no usable text.
///
/// The `description` field is normalised with [`markdown_to_plain`] before
/// inclusion so markdown links and bare URLs do not inject URL-fragment tokens
/// (`https`, host segments, path segments) into the ATS keyword set. The stored
/// `description` value is not modified — this normalisation is scoring-blob only.
pub fn posting_text_blob(
    title: &str,
    description: Option<&str>,
    requirements: Option<&[String]>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !title.trim().is_empty() {
        parts.push(title.to_string());
    }
    if let Some(d) = description {
        let plain = markdown_to_plain(d);
        if !plain.trim().is_empty() {
            parts.push(plain);
        }
    }
    if let Some(reqs) = requirements {
        for r in reqs {
            if !r.trim().is_empty() {
                parts.push(r.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}
