//! `tools/call` arguments → this CLI's own argv. Builds argv and nothing else: every value
//! is validated by the single existing `parse_verb` this reuses, so a client can never meet a
//! second argument validator with its own idea of what a valid value looks like.

use super::*;

// ── `tools/call` → argv → `parse_verb` (one validator, reused) ────────────

pub(super) fn value_as_arg(v: &Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

/// Best-effort `tools/call` arguments → this CLI's own argv. Never validates anything itself — a
/// wrong shape (a missing `url`, a non-integer `limit`, a non-object `input`) produces
/// plausible-looking argv that [`parse_verb`] then rejects with ITS OWN, already-hardened,
/// never-echo-the-value error text; this fn's only job is building that argv, not judging it.
///
/// Two conventions EVERY optional argument below follows, stated once rather than re-argued per
/// arm (which is how they drifted apart): [`value_as_arg`], never `.and_then(Value::as_str)`, so
/// a JSON NUMBER (`{"cursor": 100}`, a numeric `confirm` proof) reaches [`parse_verb`] as its
/// string form instead of vanishing as "absent" (HIGH fix, round 2 — a dropped cursor reset the
/// traversal to page 0); and `.filter(|v| !v.is_null())`, so an explicit `null` reads as ABSENT
/// rather than as the literal string `"null"` (issue #1137 — mirrors `parse_found_jobs_cursor`'s
/// own `None | Some(Value::Null)` arm; a strict schema unions optionals with `null`).
pub(super) fn tool_argv(name: &str, arguments: &Value) -> Vec<String> {
    match name {
        TOOL_BEST_MATCHES => {
            let mut argv = vec!["best-matches".to_string()];
            if let Some(limit) = arguments.get("limit").filter(|v| !v.is_null()) {
                argv.push("--limit".to_string());
                argv.push(value_as_arg(limit));
            }
            if let Some(cursor) = arguments.get("cursor").filter(|v| !v.is_null()) {
                argv.push("--cursor".to_string());
                argv.push(value_as_arg(cursor));
            }
            if let Some(query) = arguments.get("query").filter(|v| !v.is_null()) {
                argv.push("--query".to_string());
                argv.push(value_as_arg(query));
            }
            argv
        }
        TOOL_JOB => vec![
            "job".to_string(),
            arguments
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ],
        TOOL_PROFILE => vec!["profile".to_string()],
        TOOL_AUTOMATIONS => vec!["automations".to_string()],
        // Issue #1168 — `autopilotId` is now OPTIONAL (omitted spans every
        // autopilot). Forwarded as the SAME bare leading positional as
        // before when present, simply omitted when absent — `parse_found_jobs`
        // only reads the first token as `autopilotId` when it does not look
        // like a flag, so an omitted id here correctly falls through to
        // "start flag parsing at index 0".
        TOOL_FOUND_JOBS => {
            let mut argv = vec!["found-jobs".to_string()];
            if let Some(id) = arguments
                .get("autopilotId")
                .filter(|v| !v.is_null())
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                argv.push(id.to_string());
            }
            if let Some(limit) = arguments.get("limit").filter(|v| !v.is_null()) {
                argv.push("--limit".to_string());
                argv.push(value_as_arg(limit));
            }
            if let Some(cursor) = arguments.get("cursor").filter(|v| !v.is_null()) {
                argv.push("--cursor".to_string());
                argv.push(value_as_arg(cursor));
            }
            if let Some(min_score) = arguments.get("minScore").filter(|v| !v.is_null()) {
                argv.push("--min-score".to_string());
                argv.push(value_as_arg(min_score));
            }
            if let Some(country) = arguments.get("country").filter(|v| !v.is_null()) {
                argv.push("--country".to_string());
                argv.push(value_as_arg(country));
            }
            if let Some(remote) = arguments.get("remote").filter(|v| !v.is_null()) {
                argv.push("--remote".to_string());
                argv.push(value_as_arg(remote));
            }
            if let Some(applied) = arguments.get("applied").filter(|v| !v.is_null()) {
                argv.push("--applied".to_string());
                argv.push(value_as_arg(applied));
            }
            if let Some(query) = arguments.get("query").filter(|v| !v.is_null()) {
                argv.push("--query".to_string());
                argv.push(value_as_arg(query));
            }
            if arguments.get("includeDescription").and_then(Value::as_bool) == Some(true) {
                argv.push("--include-description".to_string());
            }
            argv
        }
        TOOL_CALL_READ | TOOL_CALL_REVERSIBLE | TOOL_CALL_IRREVERSIBLE => {
            let namespace = arguments
                .get("namespace")
                .and_then(Value::as_str)
                .unwrap_or("");
            let command = arguments
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("");
            let mut argv = vec!["call".to_string(), format!("{namespace}:{command}")];
            if let Some(input) = arguments.get("input") {
                argv.push("--input".to_string());
                argv.push(input.to_string());
            }
            // `confirm` is read for `call-irreversible` ONLY (MUST FIX — the other two tools'
            // schemas have no `confirm` property BY CONSTRUCTION; a misbehaving client sending
            // one anyway is silently ignored here rather than forwarded). Both conventions from
            // this fn's own doc apply (issue #1140): a NON-STRING proof is coerced, not dropped —
            // real proofs include bare numbers (`ProofSource::Count`), and dropping one answered
            // `confirmation_required` exactly as if none had been sent, collapsing the gate's
            // deliberate absent-vs-mismatch distinction.
            if name == TOOL_CALL_IRREVERSIBLE {
                if let Some(confirm) = arguments
                    .get("confirm")
                    .filter(|v| !v.is_null())
                    .map(value_as_arg)
                {
                    argv.push("--confirm".to_string());
                    argv.push(confirm);
                }
            }
            argv
        }
        _ => Vec::new(),
    }
}
