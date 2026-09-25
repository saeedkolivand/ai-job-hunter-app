//! argv → [`Verb`] for the verbs with no argument of their own beyond flags: the
//! verb dispatch itself, `best-matches`' three optional flags, and the shared
//! `--flag <true|false>` reader. Split out of `agent_cli.rs` under R8's LOC cap.

use super::*;
/// Parse `args` (excludes the program name AND the `agent` sentinel itself —
/// e.g. `["best-matches", "--limit", "10"]`). `--help`/`-h`/bare `help` are
/// intercepted by [`super::entrypoint::run`] BEFORE this is ever called, so this only ever sees
/// a real (or invalid) verb attempt. `AppError::Validation` per
/// `rust-standards`' R6 (no stringly-typed `Result<_, String>` outside
/// `error.rs`), even for this process-local, never-IPC-round-tripped parse.
pub(super) fn parse_verb(args: &[String]) -> AppResult<Verb> {
    match args.first().map(String::as_str) {
        Some("best-matches") => parse_best_matches(&args[1..]),
        Some("job") => {
            let url = args
                .get(1)
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::Validation("job requires a <url> argument".to_string()))?;
            Ok(Verb::Job {
                url: url.to_string(),
            })
        }
        Some("profile") => Ok(Verb::Profile),
        Some("automations") => Ok(Verb::Automations),
        Some("schema") => Ok(Verb::Schema),
        Some("found-jobs") => parse_found_jobs(&args[1..]),
        Some("call") => parse_call(&args[1..]),
        // Never echoes the typed token (LOW fix — security review): argv can
        // carry a path/username, and this reply lands in an agent transcript
        // — list the allowed verbs instead of the one that failed.
        Some(_) => Err(AppError::Validation(format!(
            "unknown verb (run `ajh-tauri agent --help`; expected one of: {})",
            verb_names_joined()
        ))),
        None => Err(AppError::Validation(format!(
            "missing verb (run `ajh-tauri agent --help`; expected one of: {})",
            verb_names_joined()
        ))),
    }
}

fn parse_best_matches(rest: &[String]) -> AppResult<Verb> {
    let mut limit = None;
    let mut cursor = None;
    let mut query = None;
    let mut i = 0;
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
            "--query" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--query requires a value".to_string()))?;
                query = Some(raw.to_string());
                i += 2;
            }
            // Never echoes the typed token (MINOR fix — same reasoning as
            // the unknown-verb branch above and pinned by the same kind of
            // test): argv can carry a path/username, and this reply lands
            // in an agent transcript — name the flags this verb accepts
            // instead of the one that failed.
            _ => {
                return Err(AppError::Validation(
                    "unknown argument (expected: --limit, --cursor, --query)".to_string(),
                ))
            }
        }
    }
    Ok(Verb::BestMatches {
        limit,
        cursor,
        query,
    })
}

/// Parse `--flag <true|false>`'s value into a `bool` — the one place every
/// bool flag [`super::parse_found_jobs::parse_found_jobs`] takes goes through, so `--remote maybe`
/// fails the same clear way everywhere rather than silently reading as
/// `false` (`str::parse::<bool>` already refuses anything but the exact
/// lowercase `"true"`/`"false"`, which is what this leans on).
pub(super) fn parse_bool_flag(flag: &str, raw: &str) -> AppResult<bool> {
    raw.parse::<bool>()
        .map_err(|_| AppError::Validation(format!("{flag} must be `true` or `false`")))
}

#[cfg(test)]
mod tests;
