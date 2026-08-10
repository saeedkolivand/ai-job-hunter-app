//! Tolerant JSON extraction, repair, and typed parsing for model output.
//!
//! Every provider can be ASKED for JSON — natively constrained on some, by
//! prompt discipline on the rest (see
//! [`AiProvider::complete_structured`](crate::commands::ai_provider::AiProvider::complete_structured))
//! — but a schema guarantees SHAPE at best and nothing at all on the fallback
//! path, so what comes back still has to be extracted from prose, repaired,
//! and validated (OWASP LLM05: model output is untrusted input).
//!
//! This generalizes the two hand-rolled parsers that already existed and had
//! already drifted: `salary_research::extract_json_object` (first balanced
//! `{…}`, no string awareness) and `@ajh/prompts`'s `validateAndRepair`
//! (fence strip + trailing-comma retry). One implementation, so a hardening
//! can't land on one caller and silently not the other.
//!
//! Pure (no I/O, no `AppHandle`, no logging) — every branch is unit-testable
//! directly.

use serde::de::DeserializeOwned;

/// Why a model's response could not be turned into `T`. Carries enough detail
/// for a follow-up "your last answer wasn't valid JSON, here's what broke"
/// re-ask, WITHOUT that detail being loggable by accident.
///
/// **BOTH [`Display`](std::fmt::Display) and [`Debug`](std::fmt::Debug)
/// deliberately omit [`Self::detail`]**: a serde message can quote a fragment
/// of the model's own output, and ADR-027 forbids model/prompt content
/// reaching a log line or the renderer. `{e}` AND `{e:?}` are therefore both
/// a short, content-free reason; a caller that genuinely needs the specifics
/// (the re-ask prompt) asks for [`Self::detail`] explicitly.
///
/// `Debug` is hand-written rather than derived for exactly that reason: the
/// derive prints every field, and Debug is the formatter that actually shows
/// up in the dangerous places — `tracing::error!(error = ?e)`, a bare `{e:?}`,
/// and the panic message of `.expect()`/`.unwrap()`, which reaches the crash
/// reporter (ADR-0020, default-ON). A content-free `Display` in front of a
/// leaking `Debug` protects nothing.
///
/// The two content-carrying variants hold a [`RawDetail`], not a `String`, for
/// the same reason: an enum variant's fields inherit the ENUM's visibility, so
/// a bare `String` let any caller walk straight past both formatters with
/// `if let JsonParseError::Shape(detail) = &e`.
#[derive(Clone, PartialEq, Eq)]
pub enum JsonParseError {
    /// No JSON value anywhere in the response — the model answered in prose.
    NotFound,
    /// A value opened but never closed: the response was cut off (hitting the
    /// output-token cap is the usual cause). A re-ask should shorten the
    /// request rather than just repeat it.
    Truncated,
    /// Malformed JSON that repair could not rescue.
    Syntax(RawDetail),
    /// Well-formed JSON that does not match the requested shape — a missing
    /// key, or the right key with the wrong type. The one case where the
    /// SCHEMA was ignored rather than the JSON being broken.
    Shape(RawDetail),
}

/// The parser's own message, held so that READING it is a deliberate,
/// crate-internal step. The field is private and there is no accessor: within
/// this module [`JsonParseError::detail`] destructures it, and everyone else
/// goes through the safe [`JsonParseError::reask_detail`]. A caller outside
/// this module can still match the variant (`Shape(_)`) — it just cannot get
/// at the text, which is the only part that carries model output.
///
/// `Debug` is hand-written and content-free for the same reason the enum's is:
/// this value is reachable by pattern-matching, so a derived `Debug` would
/// hand back verbatim what the enum's own formatters withhold. There is
/// deliberately no `Display` — `{d}` must not compile.
#[derive(Clone, PartialEq, Eq)]
pub struct RawDetail(String);

impl RawDetail {
    /// Wrap a parser message. Construction is safe (and `pub(crate)` so the
    /// re-ask tests can build one); it is READING that is restricted.
    pub(crate) fn new(detail: String) -> Self {
        Self(detail)
    }
}

impl std::fmt::Debug for RawDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RawDetail(<withheld>)")
    }
}

impl JsonParseError {
    /// A short, stable, content-free reason — safe for logs and the renderer.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::NotFound => "the response contained no JSON",
            Self::Truncated => "the JSON response was cut off before it finished",
            Self::Syntax(_) => "the JSON response was malformed",
            Self::Shape(_) => "the JSON response did not match the expected shape",
        }
    }

    /// The parser's own message, RAW. Never log it (see the type's doc), and
    /// never paste it into a prompt: serde quotes the offending fragment, so
    /// this string carries attacker-influenced model output and would arrive
    /// in the re-ask unfenced — the OWASP LLM01 mistake this codebase
    /// segregates against everywhere else.
    ///
    /// **Prompt callers want
    /// [`reask_detail`](Self::reask_detail) instead**, which returns the same
    /// fragment already wrapped in the crate's standard ADR-010 fence. This
    /// accessor is `pub(crate)` (the safe one is the public surface) and
    /// exists only as its input.
    pub(crate) fn detail(&self) -> &str {
        match self {
            Self::NotFound | Self::Truncated => "",
            // The one place the newtype is opened — its field is private, so
            // this module is the only one that can (see [`RawDetail`]).
            Self::Syntax(RawDetail(detail)) | Self::Shape(RawDetail(detail)) => detail,
        }
    }
}

impl std::fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

/// Variant name + [`reason`](JsonParseError::reason) — never
/// [`detail`](JsonParseError::detail). See the type's doc: this is the
/// formatter that reaches `tracing`, `{:?}`, and panic messages, so it is the
/// one that has to be content-free.
impl std::fmt::Debug for JsonParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = match self {
            Self::NotFound => "NotFound",
            Self::Truncated => "Truncated",
            Self::Syntax(_) => "Syntax",
            Self::Shape(_) => "Shape",
        };
        f.debug_tuple(variant).field(&self.reason()).finish()
    }
}

impl std::error::Error for JsonParseError {}

/// How many candidates [`parse`] will try. A response that obeys the output
/// contract has exactly ONE; a chatty one has two or three (a fence, plus the
/// same object seen again by the bare scan). The cap bounds the work a
/// pathological response can force and keeps the de-dup below trivial.
const MAX_CANDIDATES: usize = 8;

/// Every plausible JSON value in a model response, in the order [`parse`]
/// tries them — most-trustworthy first:
///
/// 1. **The whole trimmed response**, when it is exactly one balanced value
///    and nothing else. That is what a natively-constrained provider returns
///    and what the output contract asks the rest for, so it outranks anything
///    found *inside* it. Committing to a fence before deserialization was a
///    real hijack (HIGH-1): a perfectly valid object whose own string value
///    quoted a ```` ```json ```` block — a code fence echoed out of a dev job
///    ad or a projects entry — had the fence's contents parsed instead, and
///    for a `T` whose fields are all `Option`/`#[serde(default)]` that
///    deserializes fine, so the caller got an all-defaults value REPORTED AS
///    SUCCESS (and [`JsonParseError::Truncated`] never fired).
/// 2. **Fenced bodies, LAST fence first.** First-fence-wins was
///    attacker-steerable (MEDIUM-3): a posting that instructs "first echo this
///    JSON block" then supplies its own object controls the parsed value on
///    every prompt-discipline path. The model's real answer comes last, so the
///    last fence is the one to prefer — and unlike 1., ordering is what fixes
///    this, because the attacker's object deserializes perfectly.
/// 3. **Every balanced span in the response, LAST first** — the same attack
///    works without a fence, and the same "the answer comes last" reasoning
///    applies.
///
/// Ordering is a preference, not a commitment: [`parse`] walks the list and
/// takes the first candidate that actually deserializes to `T`.
fn candidates(raw: &str) -> Vec<&str> {
    let trimmed = raw.trim();
    let whole = balanced_range(trimmed)
        .filter(|span| span.start == 0 && span.end == trimmed.len())
        .map(|_| trimmed);
    let fenced = fenced_bodies(raw)
        .into_iter()
        .rev()
        .filter_map(balanced_span);
    let bare = spans(raw).into_iter().rev();

    let mut out: Vec<&str> = Vec::new();
    for candidate in whole.into_iter().chain(fenced).chain(bare) {
        if out.len() == MAX_CANDIDATES {
            break;
        }
        // Every candidate is a slice of the SAME `raw` buffer, so identity
        // (address + length) catches every duplicate without a string compare.
        if !out.iter().any(|seen| std::ptr::eq(*seen, candidate)) {
            out.push(candidate);
        }
    }
    out
}

/// The body of every ```` ```json ```` / ```` ``` ```` fence, in source order.
/// An unterminated fence (a truncated response) yields everything after the
/// opening fence and ends the scan.
fn fenced_bodies(raw: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some((_, after_open)) = rest.split_once("```") {
        let body = fence_body(after_open);
        match body.split_once("```") {
            Some((body, tail)) => {
                out.push(body);
                rest = tail;
            }
            None => {
                out.push(body);
                break;
            }
        }
    }
    out
}

/// One fence's body, given everything after its OPENING ```` ``` ````: the
/// same text minus an optional language tag on the fence's own opening line.
///
/// The tag is dropped only when that first newline is genuinely INSIDE this
/// fence — before its closing ```` ``` ```` — and only when what precedes it
/// could be a tag at all. Searching the whole remainder for a newline (the
/// first version) mis-read an INLINE fence: for ```` ```{"a":1}``` ```` followed
/// by any later line, the "tag" swallowed the real body and the fence's
/// candidate became whatever text happened to follow the NEXT newline. That is
/// not merely a lost candidate — the fenced tier outranks the bare-span tier
/// (see [`candidates`]), so an unrelated span from elsewhere in the response
/// got PROMOTED above the answer the model actually fenced, which is the
/// attacker-steerable ordering MEDIUM-3 closed.
fn fence_body(after_open: &str) -> &str {
    let close = after_open.find("```");
    match after_open.split_once('\n') {
        // `tag.len()` IS the newline's byte index, so this is "the newline
        // comes before the closing fence". The second half keeps a body that
        // merely STARTS on the opening line (```` ```{ ```` + a newline before
        // the closer) from being decapitated: a language tag is a bare word,
        // never JSON punctuation.
        Some((tag, rest))
            if close.is_none_or(|close| tag.len() < close)
                && !tag.contains(['{', '[', '"', '`']) =>
        {
            rest
        }
        _ => after_open,
    }
}

/// Every top-level balanced span in `s`, in source order. Nested values are
/// not enumerated separately (they are already inside their parent), so the
/// whole scan stays linear in `s`.
fn spans(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut offset = 0;
    while let Some(range) = balanced_range(&s[offset..]) {
        out.push(&s[offset + range.start..offset + range.end]);
        offset += range.end;
    }
    out
}

/// The first balanced `{…}`/`[…]` span in `s`, string- and escape-aware.
fn balanced_span(s: &str) -> Option<&str> {
    balanced_range(s).map(|range| &s[range])
}

/// [`balanced_span`] as a byte range, so a caller can resume the scan after
/// the span it just took (see [`spans`]) without pointer arithmetic.
fn balanced_range(s: &str) -> Option<std::ops::Range<usize>> {
    let start = s.find(['{', '['])?;
    let (open, close) = if s[start..].starts_with('{') {
        ('{', '}')
    } else {
        ('[', ']')
    };
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in s[start..].char_indices() {
        if in_string {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            // Nesting is counted per delimiter KIND: an inner `[` inside an
            // object (or vice versa) is balanced by its own closer and must
            // not be able to close the outer region.
            c if c == open => depth += 1,
            c if c == close => {
                // Cannot underflow: the loop starts ON `open`, so the first
                // iteration always takes the arm above and leaves depth >= 1.
                depth -= 1;
                if depth == 0 {
                    return Some(start..start + i + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

/// Repair the mistakes a model makes when it hand-writes JSON: a BOM or other
/// zero-width junk, a non-breaking space used as whitespace, curly/single
/// quotes used as string delimiters, a raw newline/tab inside a string, and a
/// trailing comma before `}`/`]`.
///
/// String-aware, so it only ever rewrites characters in a position where they
/// are ILLEGAL: a curly quote or NBSP *inside* a string is legal content and
/// is preserved byte-for-byte. Idempotent on already-valid JSON.
pub fn repair_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    // The delimiter that will close the string currently being scanned —
    // `None` outside a string. Tracked (rather than a bool) because a
    // curly-quoted string ends at its own closing quote, not at a `"`.
    let mut closer: Option<char> = None;
    let mut escaped = false;
    for ch in s.chars() {
        match closer {
            Some(delim) => {
                if escaped {
                    out.push(ch);
                    escaped = false;
                } else if ch == '\\' {
                    out.push(ch);
                    escaped = true;
                } else if ch == delim {
                    out.push('"');
                    closer = None;
                } else if ch == '"' {
                    // A straight quote inside a curly-quoted string would end
                    // the rewritten string early — escape it instead.
                    out.push_str("\\\"");
                } else {
                    match ch {
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        _ => out.push(ch),
                    }
                }
            }
            None => match ch {
                '"' => {
                    out.push('"');
                    closer = Some('"');
                }
                '\u{201c}' => {
                    out.push('"');
                    closer = Some('\u{201d}');
                }
                '\u{201e}' => {
                    out.push('"');
                    closer = Some('\u{201c}');
                }
                '\'' => {
                    out.push('"');
                    closer = Some('\'');
                }
                '\u{2018}' => {
                    out.push('"');
                    closer = Some('\u{2019}');
                }
                // Zero-width junk (BOM, ZWSP, word joiner) — never legal
                // JSON whitespace, and invisible in the model's own output.
                '\u{feff}' | '\u{200b}' | '\u{2060}' => {}
                // Whitespace lookalikes JSON does not accept.
                '\u{a0}' | '\u{202f}' | '\u{2007}' => out.push(' '),
                '}' | ']' => {
                    drop_trailing_comma(&mut out);
                    out.push(ch);
                }
                _ => out.push(ch),
            },
        }
    }
    out
}

/// Remove a trailing comma (and the whitespace around it) from the end of
/// `out`, called just before a `}`/`]` is appended.
fn drop_trailing_comma(out: &mut String) {
    let trimmed = out.trim_end_matches([' ', '\t', '\r', '\n']);
    if trimmed.ends_with(',') {
        let keep = trimmed.len() - 1;
        out.truncate(keep);
    }
}

/// Deserialize the model's response into `T`. The one entry point callers use.
///
/// Walks [`candidates`] and takes the first one that actually deserializes,
/// rather than committing to a single extracted span before serde has had a
/// say — the fix for a fence quoted inside a string value hijacking an
/// otherwise-valid response (HIGH-1) and for first-fence-wins being
/// attacker-steerable (MEDIUM-3).
///
/// Two passes over that same ordered list: EVERY candidate gets a clean parse
/// before ANY candidate gets [`repair_json`], so a repairable SIBLING decoy can
/// never outrank a well-formed answer, and well-formed output is never
/// rewritten.
///
/// **Containment overrides the pass split**, because otherwise the split
/// defeats the candidate ORDER that HIGH-1 (see [`candidates`]) turns on: a
/// candidate NESTED inside an earlier one is a fragment OF that candidate — the
/// ```` ```json ```` block quoted inside a string value — never a rival answer,
/// so the container's repair runs the moment its clean parse fails, BEFORE
/// anything nested inside it is tried at all. Without that, every response that
/// merely needs repairing (an unescaped newline in a string, a trailing comma,
/// smart quotes — the exact mistakes [`repair_json`] exists for) lost pass 1 and
/// handed the whole answer to whatever the model had quoted inside it.
///
/// A container that repair cannot rescue EITHER still yields to its nested
/// candidates: ordering stays a preference, not a commitment (a model that
/// mangles an outer object beyond repair and re-emits its real answer in a
/// fence inside it must still be understood), so the fallback chain runs to the
/// end in both directions.
///
/// Errors are typed ([`JsonParseError`]) so a caller can decide between
/// re-asking, shortening the request, and giving up — see each variant. The
/// reported error is the FIRST (most-trustworthy) candidate's, so the detail a
/// re-ask quotes back describes the value the model most likely meant — which
/// is why the per-candidate errors are collected by INDEX rather than by
/// arrival order (a container repaired early in pass 1 must not outrank the
/// error of a candidate that ranks above it).
pub fn parse<T: DeserializeOwned>(raw: &str) -> Result<T, JsonParseError> {
    let candidates = candidates(raw);
    // Whether a LATER (lower-priority) candidate is nested inside this one, so
    // this one's repair has to happen in pass 1 rather than pass 2.
    let contains_nested: Vec<bool> = candidates
        .iter()
        .enumerate()
        .map(|(i, candidate)| candidates[i + 1..].iter().any(|c| within(c, candidate)))
        .collect();
    let mut errors: Vec<Option<serde_json::Error>> = candidates.iter().map(|_| None).collect();

    for (i, candidate) in candidates.iter().enumerate() {
        if let Ok(value) = serde_json::from_str::<T>(candidate) {
            return Ok(value);
        }
        if contains_nested[i] {
            match serde_json::from_str::<T>(&repair_json(candidate)) {
                Ok(value) => return Ok(value),
                Err(error) => errors[i] = Some(error),
            }
        }
    }
    for (i, candidate) in candidates.iter().enumerate() {
        // Already repaired above — a container never gets a second attempt.
        if contains_nested[i] {
            continue;
        }
        match serde_json::from_str::<T>(&repair_json(candidate)) {
            Ok(value) => return Ok(value),
            Err(error) => errors[i] = Some(error),
        }
    }
    Err(match errors.into_iter().flatten().next() {
        Some(error) => classify(error),
        // Nothing balanced anywhere. An opening delimiter with no closer means
        // the response was cut off, which is a different (and differently
        // fixable) failure from "the model answered in prose".
        None if raw.contains(['{', '[']) => JsonParseError::Truncated,
        None => JsonParseError::NotFound,
    })
}

/// Whether `inner` is a byte-subrange of `outer`. Every candidate is a slice of
/// the SAME `raw` buffer (see [`candidates`]), so containment is an address
/// compare — no string search, no allocation, and no false positive from a
/// repeated substring elsewhere in the response.
fn within(inner: &str, outer: &str) -> bool {
    let (inner_start, outer_start) = (inner.as_ptr() as usize, outer.as_ptr() as usize);
    inner_start >= outer_start && inner_start + inner.len() <= outer_start + outer.len()
}

/// Map a `serde_json` failure onto the typed error. `classify()` is serde's
/// own categorization, so "the JSON was fine but the shape was wrong"
/// (`Data`) never gets reported as malformed JSON — the two need different
/// follow-ups.
fn classify(error: serde_json::Error) -> JsonParseError {
    match error.classify() {
        serde_json::error::Category::Data => {
            JsonParseError::Shape(RawDetail::new(error.to_string()))
        }
        serde_json::error::Category::Eof => JsonParseError::Truncated,
        _ => JsonParseError::Syntax(RawDetail::new(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Result {
        score: u8,
        notes: String,
    }

    // ── the first candidate ───────────────────────────────────────────────

    /// The JSON value [`parse`] tries FIRST — [`candidates`]'s head, which is
    /// the pre-hardening "commit to one extraction" behavior.
    ///
    /// A TEST-ONLY helper: this used to be a `pub fn extract_json` on the
    /// crate's forward surface with zero callers and a doc telling callers not
    /// to use it — a trap that hands a caller the exact behavior HIGH-1 was
    /// about (committing to a span before serde has had a say). The assertions
    /// below are worth keeping because they pin [`candidates`]'s ORDER, so the
    /// helper moved in here with them.
    ///
    /// String-aware — a brace or bracket inside a JSON string never opens or
    /// closes a region, which the two parsers [`parse`] replaces both got wrong
    /// (a `"note": "use {} here"` value truncated the extraction). `None` when
    /// nothing opens, and also when something opens but never closes
    /// ([`parse`] tells those two apart).
    fn extract_json(raw: &str) -> Option<&str> {
        candidates(raw).into_iter().next()
    }

    #[test]
    fn extracts_a_bare_object() {
        assert_eq!(extract_json(r#"{"a":1}"#), Some(r#"{"a":1}"#));
    }

    #[test]
    fn extracts_from_a_json_fence_and_a_bare_fence() {
        assert_eq!(extract_json("```json\n{\"a\":1}\n```"), Some("{\"a\":1}"));
        assert_eq!(extract_json("```\n[1,2]\n```"), Some("[1,2]"));
    }

    #[test]
    fn prefers_the_fenced_body_over_prose_braces_before_it() {
        // The prose `{score, notes}` is balanced but is not JSON — searching
        // the fence first is what keeps it from winning.
        let raw = "I'll return {score, notes}:\n```json\n{\"a\":1}\n```";
        assert_eq!(extract_json(raw), Some("{\"a\":1}"));
    }

    #[test]
    fn strips_prose_before_and_after_an_unfenced_object() {
        let raw = "Sure! Here it is:\n{\"a\":1}\nHope that helps.";
        assert_eq!(extract_json(raw), Some("{\"a\":1}"));
    }

    #[test]
    fn is_not_confused_by_braces_and_brackets_inside_strings() {
        // The regression the two parsers this replaces both had: a `}` inside
        // a string value ended the extraction early.
        let raw = r#"{"note":"use {} and [] here","a":1}"#;
        assert_eq!(extract_json(raw), Some(raw));
        // …including an escaped quote right before the decoy brace.
        let escaped = r#"{"note":"a \" then }","a":1}"#;
        assert_eq!(extract_json(escaped), Some(escaped));
    }

    #[test]
    fn keeps_nesting_balanced_across_both_delimiter_kinds() {
        let raw = r#"{"items":[{"id":"x"}],"n":1}"#;
        assert_eq!(extract_json(raw), Some(raw));
    }

    #[test]
    fn is_none_for_prose_and_for_a_truncated_value() {
        assert_eq!(extract_json("I cannot help with that."), None);
        assert_eq!(extract_json("```json\n{\"a\":1"), None);
    }

    // ── repair_json ───────────────────────────────────────────────────────

    #[test]
    fn repair_leaves_valid_json_untouched() {
        let valid = "{\"a\":1,\"b\":[1,2],\"c\":\"x\"}";
        assert_eq!(repair_json(valid), valid);
        // Idempotent — a second pass changes nothing either.
        assert_eq!(repair_json(&repair_json(valid)), valid);
    }

    #[test]
    fn repair_drops_trailing_commas_in_objects_and_arrays() {
        assert_eq!(repair_json("{\"a\":1,}"), "{\"a\":1}");
        assert_eq!(repair_json("[1,2, ]"), "[1,2]");
        // Nested, and with the whitespace between the comma and the closer:
        // the comma AND that whitespace go (whitespace is not significant).
        assert_eq!(repair_json("{\"a\":[1,2,],\n}"), "{\"a\":[1,2]}");
    }

    #[test]
    fn repair_rewrites_smart_and_single_quotes_used_as_delimiters() {
        assert_eq!(
            repair_json("{\u{201c}a\u{201d}: \u{201c}b\u{201d}}"),
            "{\"a\": \"b\"}"
        );
        assert_eq!(repair_json("{'a': 'b'}"), "{\"a\": \"b\"}");
    }

    #[test]
    fn repair_preserves_a_curly_quote_that_is_real_string_content() {
        // The whole point of being string-aware: an apostrophe or curly quote
        // INSIDE a value is content, not a delimiter.
        let raw = "{\"a\": \"it\u{2019}s \u{201c}fine\u{201d}\"}";
        assert_eq!(repair_json(raw), raw);
    }

    #[test]
    fn repair_strips_a_bom_and_normalizes_nbsp_outside_strings() {
        assert_eq!(repair_json("\u{feff}{\u{a0}\"a\":1}"), "{ \"a\":1}");
        // …but an NBSP inside a string is legal content and stays.
        assert_eq!(repair_json("{\"a\":\"x\u{a0}y\"}"), "{\"a\":\"x\u{a0}y\"}");
    }

    #[test]
    fn repair_escapes_a_raw_newline_inside_a_string() {
        assert_eq!(repair_json("{\"a\":\"one\ntwo\"}"), "{\"a\":\"one\\ntwo\"}");
    }

    // ── parse ─────────────────────────────────────────────────────────────

    #[test]
    fn parses_a_fenced_prefixed_response() {
        let raw = "Here you go:\n```json\n{\"score\": 72, \"notes\": \"ok\"}\n```\nDone.";
        assert_eq!(
            parse::<Result>(raw).expect("parses"),
            Result {
                score: 72,
                notes: "ok".to_string()
            }
        );
    }

    #[test]
    fn parses_only_after_repair_when_the_model_hand_wrote_the_json() {
        let raw = "{\u{201c}score\u{201d}: 72, \u{201c}notes\u{201d}: \u{201c}ok\u{201d},}";
        assert_eq!(
            parse::<Result>(raw).expect("repairs then parses"),
            Result {
                score: 72,
                notes: "ok".to_string()
            }
        );
    }

    #[test]
    fn reports_not_found_for_a_prose_only_answer() {
        assert_eq!(
            parse::<Result>("I'm sorry, I can't do that.").unwrap_err(),
            JsonParseError::NotFound
        );
    }

    #[test]
    fn reports_truncated_for_a_response_cut_off_mid_value() {
        // Distinct from NotFound on purpose: the fix is a shorter request, not
        // a re-ask with the same prompt.
        assert_eq!(
            parse::<Result>("{\"score\": 72, \"notes\": \"ok").unwrap_err(),
            JsonParseError::Truncated
        );
    }

    #[test]
    fn reports_shape_not_syntax_when_the_json_is_valid_but_wrong() {
        // The schema was ignored, not the JSON format — different follow-up.
        let err = parse::<Result>(r#"{"score": "seventy", "notes": "ok"}"#).unwrap_err();
        assert!(
            matches!(err, JsonParseError::Shape(_)),
            "expected a shape error, got {err:?}"
        );
        let missing = parse::<Result>(r#"{"score": 72}"#).unwrap_err();
        assert!(
            matches!(missing, JsonParseError::Shape(_)),
            "a missing key is a shape error, got {missing:?}"
        );
    }

    #[test]
    fn reports_syntax_for_json_repair_cannot_rescue() {
        let err = parse::<Result>(r#"{"score": 72 "notes": "ok"}"#).unwrap_err();
        assert!(
            matches!(err, JsonParseError::Syntax(_)),
            "expected a syntax error, got {err:?}"
        );
    }

    #[test]
    fn display_never_leaks_model_content_but_detail_keeps_it_for_the_re_ask() {
        // ADR-027: `{e}` in a log line must not carry the model's own text.
        let err = parse::<Result>(r#"{"score": "SECRET-VALUE", "notes": "ok"}"#).unwrap_err();
        assert!(
            !err.to_string().contains("SECRET-VALUE"),
            "Display leaked model content: {err}"
        );
        assert_eq!(err.to_string(), err.reason());
        assert!(
            err.detail().contains("SECRET-VALUE"),
            "detail must keep the specifics for a re-ask: {}",
            err.detail()
        );
    }

    #[test]
    fn debug_never_leaks_model_content_either() {
        // HIGH-2: `Display` was content-free but `#[derive(Debug)]` printed the
        // withheld fragment verbatim — and Debug is what `tracing::error!(error
        // = ?e)`, any `{e:?}`, and an `.expect()` panic message (which reaches
        // the crash reporter) actually print. Mutation check: restore
        // `#[derive(Debug)]` on `JsonParseError` and this fails — on the
        // equality assertion now rather than the leak one, because the
        // `RawDetail` payload is content-free under Debug too (defense in
        // depth: the derive prints `Shape(RawDetail(<withheld>))`).
        let err = parse::<Result>(r#"{"score": "SECRET-VALUE", "notes": "ok"}"#).unwrap_err();
        let debug = format!("{err:?}");
        assert!(!debug.contains("SECRET-VALUE"), "Debug leaked: {debug}");
        assert_eq!(debug, format!("Shape({:?})", err.reason()));
    }

    #[test]
    fn the_payload_a_caller_can_reach_by_pattern_matching_is_content_free_too() {
        // MEDIUM: the two content-carrying variants held a bare `String`, and
        // a variant's fields inherit the ENUM's visibility — so any caller
        // could sidestep the content-free `Display`/`Debug` entirely with
        // `if let JsonParseError::Shape(detail) = &e { … }` and log the model's
        // own output. The payload is now a newtype whose field is private, so
        // the only way through is the `pub(crate)` `detail()` (or the fenced
        // `reask_detail()`), and what a caller CAN reach formats content-free.
        // Mutation check: `#[derive(Debug)]` on `RawDetail` and this fails.
        let err = parse::<Result>(r#"{"score": "SECRET-VALUE", "notes": "ok"}"#).unwrap_err();
        let (JsonParseError::Shape(raw) | JsonParseError::Syntax(raw)) = &err else {
            panic!("expected a content-carrying variant, got {err:?}");
        };
        let debug = format!("{raw:?}");
        assert!(
            !debug.contains("SECRET-VALUE"),
            "payload Debug leaked: {debug}"
        );
    }

    #[test]
    fn the_raw_detail_payload_field_stays_private() {
        // The one property of this fix no runtime assertion can observe:
        // `RawDetail`'s field must stay PRIVATE, because that — not the
        // hand-written `Debug` — is what makes reading the raw message
        // impossible outside this module. A source pin is the cheapest guard
        // that fails on the mutation (same `include_str!` compile-time-pin
        // convention as `extension_bridge::answer_rewrite`'s translation
        // parity test); `include_str!` also makes rustc track the file, so the
        // test can never read a stale copy.
        const SRC: &str = include_str!("json.rs");
        // Assembled at runtime, never written out as one literal: an inline
        // needle would appear in THIS line and satisfy its own scan (it did,
        // on the first run — the test passed before the newtype existed).
        let private_field = format!("pub struct RawDetail({}String);", "");
        assert!(
            SRC.contains(&private_field),
            "RawDetail's field must stay private — `pub`/`pub(crate)` on it \
             re-opens the pattern-match leak the newtype exists to close"
        );
    }

    // ── candidate ordering (HIGH-1 / MEDIUM-3) ────────────────────────────

    /// A `T` whose every field has a serde default — the shape that turns a
    /// hijacked extraction into SILENT DATA LOSS rather than a parse error.
    #[derive(Debug, Deserialize, PartialEq, Default)]
    #[serde(default)]
    struct Lenient {
        note: String,
        score: u8,
    }

    #[test]
    fn a_fence_quoted_inside_a_string_value_cannot_hijack_a_valid_response() {
        // HIGH-1: the whole response is ONE valid JSON object whose `note`
        // value happens to quote a ```` ```json ```` block — a code fence
        // echoed out of a dev job ad or a projects entry. Committing to the
        // fence before deserialization parsed the `{}` INSIDE the string, and
        // because every field of `Lenient` has a default that came back `Ok`
        // with all-defaults: data loss reported as success.
        let raw = r#"{"note":"see ```json\n{}\n``` above","score":7}"#;
        assert_eq!(
            parse::<Lenient>(raw).expect("the whole response is the answer"),
            Lenient {
                note: "see ```json\n{}\n``` above".to_string(),
                score: 7,
            }
        );
        assert_eq!(extract_json(raw), Some(raw));
    }

    #[test]
    fn a_nested_fence_cannot_hijack_a_response_that_merely_needs_repair() {
        // HIGH-1's guard is the candidate ORDER, but the clean-pass/repair-pass
        // split used to defeat it: the container lost pass 1 for needing ANY
        // repair at all, and the fence quoted inside its own string value —
        // clean by construction — won outright. Three ways for a container to
        // fail the clean pass, one line each; all three were a silent
        // all-defaults `Lenient` before containment ordering. Mutation check:
        // drop the `contains_nested` pass-1 repair in `parse` and all three
        // fail.
        let expected = Lenient {
            note: "see ```json\n{}\n``` above".to_string(),
            score: 7,
        };
        // 1. a RAW newline inside the string (the model never escaped it) —
        //    note that a straight-quoted container can only ever nest a
        //    quote-free decoy: an inner `"` would end the container's string,
        //    and an escaped `\"` leaves the fence body unbalanced, so `{}` (the
        //    all-defaults data-loss shape HIGH-1 describes) is the loudest
        //    decoy this shape admits.
        let raw_newline = "{\"note\":\"see ```json\n{}\n``` above\",\"score\":7}";
        assert!(
            candidates(raw_newline).len() > 1,
            "the nested fence must really be a candidate, else this pins nothing"
        );
        assert_eq!(parse::<Lenient>(raw_newline).expect("repairs"), expected);

        // 2. a trailing comma.
        let trailing_comma = r#"{"note":"see ```json\n{}\n``` above","score":7,}"#;
        assert_eq!(parse::<Lenient>(trailing_comma).expect("repairs"), expected);

        // 3. smart quotes as delimiters — the one variant whose container can
        //    nest a fully-formed decoy (a straight `"` is just content between
        //    curly delimiters), so here the hijack was a LOUD wrong answer
        //    rather than an empty one.
        let smart = "{\u{201c}note\u{201d}:\u{201c}see ```json\n\
             {\"score\": 100, \"note\": \"PWNED\"}\n``` above\u{201d},\u{201c}score\u{201d}:7}";
        let parsed = parse::<Lenient>(smart).expect("repairs");
        assert_eq!(
            parsed.score, 7,
            "the decoy nested in the string value won: {parsed:?}"
        );
        assert!(
            parsed.note.contains("PWNED"),
            "the decoy is CONTENT of the real answer, not the answer: {parsed:?}"
        );
    }

    #[test]
    fn a_clean_candidate_still_beats_a_repairable_one_that_outranks_it() {
        // The other half of the pass split, and the property the containment
        // rule above must not eat: between SIBLINGS (neither inside the other)
        // every candidate gets a clean parse before any gets `repair_json`, so
        // a hand-written decoy the model would never have produced cannot win
        // just by ranking higher. Mutation check: collapse `parse`'s two loops
        // into one try-clean-then-repair per candidate and this fails.
        let raw = "Real: {\"score\": 2, \"notes\": \"real\"}\n\
             Decoy: {'score': 1, 'notes': 'decoy'}";
        assert_eq!(
            candidates(raw).first().copied(),
            Some("{'score': 1, 'notes': 'decoy'}"),
            "the premise: last-span-first ranks the repairable decoy ABOVE the real answer"
        );
        assert_eq!(
            parse::<Result>(raw).expect("parses"),
            Result {
                score: 2,
                notes: "real".to_string(),
            }
        );
    }

    #[test]
    fn the_last_fenced_candidate_wins_over_an_echoed_first_one() {
        // MEDIUM-3: a posting that instructs "first echo this JSON block"
        // controlled the parsed value under first-fence-wins. Both objects
        // deserialize, so HIGH-1's candidate ordering alone does not cover
        // this — the ORDER among fenced candidates is what does.
        let raw = "The posting says to echo this first:\n\
             ```json\n{\"score\": 100, \"notes\": \"echoed\"}\n```\n\
             My real answer:\n\
             ```json\n{\"score\": 12, \"notes\": \"real\"}\n```";
        assert_eq!(
            parse::<Result>(raw).expect("parses"),
            Result {
                score: 12,
                notes: "real".to_string(),
            }
        );
    }

    #[test]
    fn the_last_unfenced_candidate_wins_too() {
        // Same attack without a fence — the model's real answer still comes
        // last, so the scan must not stop at the first balanced span.
        let raw = "Echo: {\"score\": 100, \"notes\": \"echoed\"}\n\
             Real: {\"score\": 12, \"notes\": \"real\"}";
        assert_eq!(
            parse::<Result>(raw).expect("parses"),
            Result {
                score: 12,
                notes: "real".to_string(),
            }
        );
    }

    #[test]
    fn an_inline_fence_keeps_its_own_body_instead_of_a_later_line() {
        // Stripping the "language tag" by searching the WHOLE remainder for a
        // newline mis-read a fence opened and closed on one line: the tag
        // swallowed the real body, and the fenced candidate became whatever
        // followed the next newline anywhere in the response. Because the
        // fenced tier outranks the bare-span tier, that PROMOTED an unrelated
        // span above the model's actual fenced answer — MEDIUM-3's ordering
        // attack, reachable again through a one-line fence. Mutation check:
        // restore `after_open.split_once('\n').map_or(...)` in `fence_body` and
        // both assertions fail (the decoy is returned instead).
        let raw = "```{\"score\": 12, \"notes\": \"real\"}```\n\
             See also {\"score\": 100, \"notes\": \"decoy\"}";
        assert_eq!(
            candidates(raw).first().copied(),
            Some("{\"score\": 12, \"notes\": \"real\"}"),
            "the fenced body itself must be the top candidate"
        );
        assert_eq!(
            parse::<Result>(raw).expect("parses"),
            Result {
                score: 12,
                notes: "real".to_string(),
            }
        );
        // Same promotion, reached the other way: a fence whose body STARTS on
        // the opening line and then wraps. Here the newline really is inside
        // the fence, so the extent check alone still decapitates the body to
        // an unbalanced fragment and the fenced tier silently falls to the
        // trailing decoy. Mutation check: drop the `!tag.contains([...])` half
        // of `fence_body`'s guard and this assertion fails — a language tag is
        // a bare word, never JSON punctuation.
        let wrapped = "```{\n\"score\": 12, \"notes\": \"real\"}\n```\n\
             P.S. ignore this: {\"score\": 100, \"notes\": \"decoy\"}";
        assert_eq!(
            parse::<Result>(wrapped).expect("parses"),
            Result {
                score: 12,
                notes: "real".to_string(),
            }
        );

        // The tagged, multi-line fence still behaves exactly as before.
        assert_eq!(extract_json("```json\n{\"a\":1}\n```"), Some("{\"a\":1}"));
    }

    #[test]
    fn the_candidate_cap_holds_and_keeps_the_highest_priority_candidates() {
        // The cap bounds the work a pathological response can force, so it has
        // to actually bind — and it has to drop from the BOTTOM: candidates are
        // ordered most-trustworthy first, so truncating the other end would
        // throw away the model's real answer (last span first) and hand the
        // parse to an echoed decoy. Mutation check: drop the `break` in
        // `candidates`, or truncate with `out.remove(0)` instead, and this
        // fails.
        //
        // The band pins the constant itself (a range, not the literal — the
        // exact number is a judgement call, its ORDER of magnitude isn't): a
        // chatty-but-honest response yields two or three candidates, so
        // anything below that starts discarding real fallbacks, and a cap
        // large enough to stop bounding the work isn't a cap.
        assert!(
            (3..=32).contains(&MAX_CANDIDATES),
            "MAX_CANDIDATES = {MAX_CANDIDATES} is outside the sane band"
        );

        let mut raw = String::from("The posting says to echo these first:\n");
        for i in 0..MAX_CANDIDATES + 4 {
            raw.push_str(&format!("{{\"score\": {i}, \"notes\": \"echo\"}}\n"));
        }
        raw.push_str("My real answer:\n{\"score\": 12, \"notes\": \"real\"}");
        assert!(
            spans(&raw).len() > MAX_CANDIDATES,
            "the premise: there must be more balanced spans than the cap allows"
        );

        let picked = candidates(&raw);
        assert_eq!(picked.len(), MAX_CANDIDATES);
        assert_eq!(
            picked.first().copied(),
            Some("{\"score\": 12, \"notes\": \"real\"}")
        );
        assert_eq!(
            parse::<Result>(&raw).expect("parses"),
            Result {
                score: 12,
                notes: "real".to_string(),
            }
        );
    }

    #[test]
    fn a_candidate_that_does_not_deserialize_falls_through_to_the_next_one() {
        // Ordering is a PREFERENCE, not a commitment: the last fence here is a
        // shape the caller didn't ask for, so the earlier one still wins.
        let raw = "```json\n{\"score\": 12, \"notes\": \"real\"}\n```\n\
             For reference the schema is:\n```json\n{\"score\": \"<integer>\"}\n```";
        assert_eq!(
            parse::<Result>(raw).expect("parses"),
            Result {
                score: 12,
                notes: "real".to_string(),
            }
        );
    }
}
