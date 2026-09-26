//! `agent found-jobs`'s own argv: the OPTIONAL `autopilotId` positional and the
//! filter flags, including the blank-selector refusal shared verbatim with the MCP tool one hop
//! further in. Split out of `agent_cli.rs` under R8's LOC cap.

use super::*;
/// A present-but-empty `<autopilotId>` positional (the canonical unset-shell-
/// variable shape, `agent found-jobs "$AP_ID"` with `AP_ID` unset) must
/// refuse with the SAME blank-selector message
/// `found_jobs::parse_autopilot_id_arg`/`mcp::classify_tool_call` give the
/// identical mistake one hop further in (round 3 fix, B3-r3-F9) — before
/// this fix, an empty first token fell to `(None, 0)` below, `omitted`, so
/// the flag-parsing loop started AT that same empty token and hit the
/// catch-all "unknown argument" arm, steering a caller toward dropping the
/// positional entirely (the exact spanning-scope mistake `agent-cli-standards`
/// says an empty selector must never fall into). A whitespace-only id (`" "`)
/// is left to the existing downstream refusal — it survives this positional
/// check (not empty) and is caught by `parse_autopilot_id_arg`'s own trim.
const BLANK_FOUND_JOBS_AUTOPILOT_ID_MESSAGE: &str =
    "autopilotId must be a non-empty id, not blank or flag-shaped — omit the positional \
     entirely to span every autopilot";

/// Parse `found-jobs`' own args: `[<autopilotId>] [--limit <n>] [--cursor <c>]
/// [--min-score <n>] [--country <s>] [--remote <bool>] [--applied <bool>]
/// [--query <q>] [--include-description]`. `autopilotId` is now OPTIONAL
/// (issue #1168) — the first token is read as one only when it does not look
/// like a flag (does not start with `--`); omitted entirely, flag parsing
/// simply starts at index 0. Mirrors [`super::parse::parse_best_matches`]'s flag-parsing
/// loop. Never echoes an unknown flag's raw token (same reasoning as
/// [`super::parse::parse_best_matches`]'s own comment).
pub(super) fn parse_found_jobs(rest: &[String]) -> AppResult<Verb> {
    let (autopilot_id, mut i) = match rest.first() {
        Some(s) if s.is_empty() => {
            return Err(AppError::Validation(
                BLANK_FOUND_JOBS_AUTOPILOT_ID_MESSAGE.to_string(),
            ))
        }
        Some(s) if !s.starts_with("--") => (Some(s.clone()), 1),
        _ => (None, 0),
    };

    let mut limit = None;
    let mut cursor = None;
    let mut min_score = None;
    let mut country = None;
    let mut remote = None;
    let mut applied = None;
    let mut query = None;
    let mut include_description = false;
    while i < rest.len() {
        match rest[i].as_str() {
            "--limit" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--limit requires a value".to_string()))?;
                limit = Some(raw.parse::<u64>().map_err(|_| {
                    AppError::Validation("--limit must be a non-negative integer".to_string())
                })?);
                i += 2;
            }
            "--cursor" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--cursor requires a value".to_string()))?;
                cursor = Some(raw.to_string());
                i += 2;
            }
            "--min-score" => {
                let raw = rest.get(i + 1).ok_or_else(|| {
                    AppError::Validation("--min-score requires a value".to_string())
                })?;
                let parsed = raw.parse::<f64>().map_err(|_| {
                    AppError::Validation("--min-score must be a number".to_string())
                })?;
                // B3-r1-F3 — `f64::parse` accepts `"1e400"`/`"inf"`/`"nan"`
                // as valid non-finite values; `json!(non_finite)` then
                // serializes to `null`, which the resource-side filter reads
                // as "absent" and silently drops. Refused here so the filter
                // either applies or the call fails, never a third, quiet
                // option.
                if !parsed.is_finite() {
                    return Err(AppError::Validation(
                        "--min-score must be a finite number".to_string(),
                    ));
                }
                min_score = Some(parsed);
                i += 2;
            }
            "--country" => {
                let raw = rest.get(i + 1).ok_or_else(|| {
                    AppError::Validation("--country requires a value".to_string())
                })?;
                country = Some(raw.to_string());
                i += 2;
            }
            "--remote" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--remote requires a value".to_string()))?;
                remote = Some(parse_bool_flag("--remote", raw)?);
                i += 2;
            }
            "--applied" => {
                let raw = rest.get(i + 1).ok_or_else(|| {
                    AppError::Validation("--applied requires a value".to_string())
                })?;
                applied = Some(parse_bool_flag("--applied", raw)?);
                i += 2;
            }
            "--query" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--query requires a value".to_string()))?;
                query = Some(raw.to_string());
                i += 2;
            }
            "--include-description" => {
                include_description = true;
                i += 1;
            }
            _ => {
                return Err(AppError::Validation(
                    "unknown argument (expected: --limit, --cursor, --min-score, --country, \
                     --remote, --applied, --query, --include-description)"
                        .to_string(),
                ))
            }
        }
    }
    Ok(Verb::FoundJobs {
        autopilot_id,
        limit,
        cursor,
        min_score,
        country,
        remote,
        applied,
        query,
        include_description,
    })
}

#[cfg(test)]
mod tests;
