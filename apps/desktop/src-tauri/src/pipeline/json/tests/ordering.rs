//! The HIGH-1 (a fence quoted inside a valid response's own string value) and
//! MEDIUM-3 (first-fence/first-span-wins is attacker-steerable) candidate-
//! ordering hardening.

use serde::Deserialize;

use super::super::*;
use super::support::{extract_json, Result};

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
