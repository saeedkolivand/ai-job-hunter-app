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
/// **[`Display`](std::fmt::Display) deliberately omits [`Self::detail`]**: a
/// serde message can quote a fragment of the model's own output, and ADR-027
/// forbids model/prompt content reaching a log line or the renderer. `{e}` is
/// therefore always a short, content-free reason; a caller that genuinely
/// needs the specifics (the re-ask prompt) asks for [`Self::detail`]
/// explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonParseError {
    /// No JSON value anywhere in the response — the model answered in prose.
    NotFound,
    /// A value opened but never closed: the response was cut off (hitting the
    /// output-token cap is the usual cause). A re-ask should shorten the
    /// request rather than just repeat it.
    Truncated,
    /// Malformed JSON that repair could not rescue.
    Syntax(String),
    /// Well-formed JSON that does not match the requested shape — a missing
    /// key, or the right key with the wrong type. The one case where the
    /// SCHEMA was ignored rather than the JSON being broken.
    Shape(String),
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

    /// The parser's own message, for a re-ask prompt ONLY. May quote a
    /// fragment of the model's output — never log it (see the type's doc).
    pub fn detail(&self) -> &str {
        match self {
            Self::NotFound | Self::Truncated => "",
            Self::Syntax(detail) | Self::Shape(detail) => detail,
        }
    }
}

impl std::fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for JsonParseError {}

/// Find the JSON value in a model response: the first balanced `{…}` or `[…]`,
/// searched inside a Markdown code fence when there is one (so prose before
/// the fence can't win) and across the whole response otherwise.
///
/// String-aware — a brace or bracket inside a JSON string never opens or
/// closes a region, which the two parsers this replaces both got wrong (a
/// `"note": "use {} here"` value truncated the extraction). Returns `None`
/// when nothing opens, and also when something opens but never closes
/// ([`parse`] tells those two apart).
pub fn extract_json(raw: &str) -> Option<&str> {
    fenced_body(raw)
        .and_then(balanced_span)
        .or_else(|| balanced_span(raw))
}

/// The body of the first ```` ```json ```` / ```` ``` ```` fence, or `None`
/// when the response isn't fenced. An unterminated fence (a truncated
/// response) yields everything after the opening fence.
fn fenced_body(raw: &str) -> Option<&str> {
    let after_open = raw.split_once("```")?.1;
    // Drop an optional language tag on the opening fence's own line.
    let body = after_open.split_once('\n').map_or(after_open, |(_, r)| r);
    Some(body.split_once("```").map_or(body, |(b, _)| b))
}

/// The first balanced `{…}`/`[…]` span in `s`, string- and escape-aware.
fn balanced_span(s: &str) -> Option<&str> {
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
                    return Some(&s[start..start + i + ch.len_utf8()]);
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

/// Extract → parse → repair → parse. The one entry point callers use.
///
/// The repair pass runs only after a clean parse has already failed, so
/// well-formed output is never rewritten. Errors are typed
/// ([`JsonParseError`]) so a caller can decide between re-asking, shortening
/// the request, and giving up — see each variant.
pub fn parse<T: DeserializeOwned>(raw: &str) -> Result<T, JsonParseError> {
    let Some(candidate) = extract_json(raw) else {
        // Nothing balanced. An opening delimiter with no closer means the
        // response was cut off, which is a different (and differently
        // fixable) failure from "the model answered in prose".
        return Err(if raw.contains(['{', '[']) {
            JsonParseError::Truncated
        } else {
            JsonParseError::NotFound
        });
    };
    if let Ok(value) = serde_json::from_str::<T>(candidate) {
        return Ok(value);
    }
    serde_json::from_str::<T>(&repair_json(candidate)).map_err(classify)
}

/// Map a `serde_json` failure onto the typed error. `classify()` is serde's
/// own categorization, so "the JSON was fine but the shape was wrong"
/// (`Data`) never gets reported as malformed JSON — the two need different
/// follow-ups.
fn classify(error: serde_json::Error) -> JsonParseError {
    match error.classify() {
        serde_json::error::Category::Data => JsonParseError::Shape(error.to_string()),
        serde_json::error::Category::Eof => JsonParseError::Truncated,
        _ => JsonParseError::Syntax(error.to_string()),
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

    // ── extract_json ──────────────────────────────────────────────────────

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
}
