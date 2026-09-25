//! `agent call <ns>:<command>`'s own argv: the `--input`/`--confirm` pair and the
//! purely local target-shape guards, which resolve without the app running exactly as `--help`
//! does. Split out of `agent_cli.rs` under R8's LOC cap.

use super::*;
/// Parse `call`'s own args: `<namespace>:<command> [--input '<json>']
/// [--confirm '<value>']`. Both target-parsing failure modes are pure ARGV
/// shape — no policy-table lookup, no network — so they resolve the SAME way
/// `--help` does: without the app running. Whether `<namespace>:<command>`
/// names a real, dispatchable command (and which class it is) is decided
/// server-side (`agent_call::dispatch`), never guessed here — this fn only
/// rejects a token that couldn't possibly be one.
///
/// `--confirm`'s raw value is NEVER echoed in any error here, and is carried
/// only as far as [`Verb::payload`] — this client never logs it, never
/// prints it outside the one frame it belongs on (path privacy AND ADR-038
/// §4's own "the caller's own data" rule apply equally to this flag).
pub(super) fn parse_call(rest: &[String]) -> AppResult<Verb> {
    let target = rest
        .first()
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::Validation("call requires a <namespace>:<command> argument".to_string())
        })?;
    let (namespace, command) = target
        .split_once(':')
        .filter(|(n, c)| !n.is_empty() && !c.is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "call's first argument must be <namespace>:<command> (see `agent schema` or the \
                 MCP `commands` tool)"
                    .to_string(),
            )
        })?;

    let mut input = json!({});
    let mut confirm = None;
    let mut i = 1;
    while i < rest.len() {
        match rest[i].as_str() {
            "--input" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--input requires a value".to_string()))?;
                // Never echoes `raw` (path privacy — the value may carry a
                // path or other sensitive content the caller typed).
                let parsed: Value = serde_json::from_str(raw)
                    .map_err(|_| AppError::Validation("--input must be valid JSON".to_string()))?;
                if !parsed.is_object() {
                    return Err(AppError::Validation(
                        "--input must be a JSON object".to_string(),
                    ));
                }
                input = parsed;
                i += 2;
            }
            "--confirm" => {
                let raw = rest.get(i + 1).ok_or_else(|| {
                    AppError::Validation("--confirm requires a value".to_string())
                })?;
                // Never echoed anywhere — this IS the ceremony's proof
                // value (ADR-038 §4).
                confirm = Some(raw.to_string());
                i += 2;
            }
            _ => {
                return Err(AppError::Validation(
                    "unknown argument (expected: --input, --confirm)".to_string(),
                ))
            }
        }
    }
    Ok(Verb::Call {
        namespace: namespace.to_string(),
        command: command.to_string(),
        input,
        confirm,
    })
}

#[cfg(test)]
mod tests;
