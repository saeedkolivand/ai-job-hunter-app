//! Dispatch-time input-key validation against the generated
//! [`super::super::agent_cli::catalogue::CATALOGUE`] (issues #1163, #1158, #1160). Called as
//! [`dispatch`]'s own FIRST check, before [`gate`] — the ordering fix for #1160: a missing
//! required key must refuse before `confirmation_required` is ever emitted, so an approved
//! irreversible delete can never die on a missing arg afterwards (a wrong wrapper guess or a
//! stray key used to reach the real command's own deserializer instead, or worse — for an
//! `Option<T>` field, simply vanish and report `success: true`, issues #1158's whole point).
//!
//! A command absent from the catalogue (an `invoke()` call the generator could not parse with
//! confidence, or one with no call site at all) is left UNCHECKED — this is the generator's own
//! documented contract (`catalogue.rs`'s module doc), not a gap this file papers over: nothing
//! here can validate a shape it was never told, and a command this generator could not resolve
//! stays exactly as permissive as it was before this pass. `policy::tests`' own catalogue-coverage
//! test is what keeps that allowlist from growing silently.

use serde_json::{Map, Value};

use super::super::agent_cli::catalogue::{CatalogueArg, CATALOGUE};
use super::super::agent_cli::policy::Effect;
use super::reshape::PAGINATED_LIST_COMMANDS;
use super::Refusal;

/// `command`'s own declared arguments, or `None` when it is absent from the generated catalogue —
/// the ONE signal [`check_input`] needs to skip validation entirely for it.
fn entry_for(command: &str) -> Option<&'static [CatalogueArg]> {
    CATALOGUE
        .iter()
        .find(|entry| entry.command == command)
        .map(|entry| entry.args)
}

/// Comma-joined declared key names, for a refusal detail that tells the caller what WOULD have
/// worked rather than only what didn't.
fn declared_keys(args: &[CatalogueArg]) -> String {
    args.iter()
        .map(|arg| arg.name)
        .collect::<Vec<_>>()
        .join(", ")
}

fn invalid_input(message: String) -> Refusal {
    Refusal::InvalidInput(message)
}

/// A caller-supplied JSON key, fenced and capped before it enters an
/// [`invalid_input`] detail (HIGH — security review). `given.keys()` is
/// attacker-controlled text bounded only by the bridge's own incoming frame
/// cap (8 MiB) and, unlike every other echoed identifier in this crate,
/// carried NO fence and NO cap: a caller could get a `detail` that (a) echoes
/// third-party text back in the server's OWN voice, outside any fence,
/// laundering exactly what [`Refusal::InvokeError`]'s fencing exists to
/// prevent, and (b) is large enough to blow the reply past
/// [`super::enforce_frame_cap`]. Same primitive `InvokeError` already uses,
/// not a second one.
fn fenced_key(key: &str) -> String {
    crate::prompt_fence::fenced("job_posting", key, crate::prompt_fence::JOB_CAP)
}

/// Validate `input`'s top-level keys (and, for a wrapper key whose type the generator could
/// resolve, that key's own object one level down) against `command`'s declared input contract.
/// `Ok(())` for a command absent from the catalogue — see this module's own doc.
///
/// `input` need not be a JSON object: a non-object value (or an absent one, which
/// [`super::handle_agent_call`] already defaults to `{}` before this is ever reached) is treated
/// as carrying no top-level keys at all — every required key on it is reported missing, and there
/// is nothing to walk for an unknown-key check, which degrades to a no-op rather than a panic.
pub(super) fn check_input(command: &str, input: &Value) -> Result<(), Refusal> {
    let Some(args) = entry_for(command) else {
        return Ok(());
    };
    // This layer's OWN paging keys (`agent_call::reshape::take_list_page_args`) are removed from
    // `input` AFTER this check runs (`dispatch_direct` calls it later in the pipeline), so a
    // paged row with zero declared args — both real rows today, `applications_list` and
    // `ai_generations_list` — must not see its own `limit`/`cursor` refused as unknown.
    let paged = PAGINATED_LIST_COMMANDS.contains(&command);
    let empty = Map::new();
    let given = input.as_object().unwrap_or(&empty);

    for key in given.keys() {
        if paged && (key == "limit" || key == "cursor") {
            continue;
        }
        if !args.iter().any(|arg| arg.name == key) {
            return Err(invalid_input(format!(
                "unknown key `{}` for {command} — declared keys: {}",
                fenced_key(key),
                declared_keys(args)
            )));
        }
    }

    for arg in args {
        if arg.required && !given.contains_key(arg.name) {
            return Err(invalid_input(format!(
                "missing required key `{}` for {command} — declared keys: {}",
                arg.name,
                declared_keys(args)
            )));
        }
        // `None` (not a wrapper) or `Some(&[])` (a wrapper this generator could not resolve —
        // see `CatalogueArg::fields`'s own doc) both mean "nothing to check a nested key
        // against"; only a genuinely resolved, non-empty field list can refuse one.
        let Some(fields) = arg.fields.filter(|f| !f.is_empty()) else {
            continue;
        };
        let Some(nested) = given.get(arg.name).and_then(Value::as_object) else {
            continue;
        };
        for key in nested.keys() {
            if !fields.contains(&key.as_str()) {
                return Err(invalid_input(format!(
                    "unknown key `{}.{}` for {command} — declared keys under `{}`: {}",
                    arg.name,
                    fenced_key(key),
                    arg.name,
                    fields.join(", ")
                )));
            }
        }
    }

    Ok(())
}

/// A required wrapper key whose value is an empty object `{}` — refused on a mutating row
/// (A1-r1-SEC-2 MEDIUM). [`CatalogueArg::fields`] carries no per-nested-field required marker (the
/// underlying Zod schema knows it; threading it through is a larger follow-up than this fix), so
/// this cannot tell "every field inside is legitimately optional" from "the caller sent nothing at
/// all" — but on a `Reversible`/`Irreversible` row, an empty required wrapper reaching an all-
/// `Option` request struct is almost always the #1158 symptom (an empty
/// `applications_save_from_posting` row answering `success: true`), never a deliberate no-op, so
/// this refuses it outright rather than letting `check_input`'s membership-only walk wave it
/// through. Never applied to a `Read` row (a filter-shaped wrapper, e.g.
/// `scrape_list_interactions`'s `filter`, can legitimately be sent empty to mean "no filter") or an
/// uncatalogued command (nothing here to check — see this module's own doc). Deliberately a
/// SEPARATE fn from [`check_input`], not folded into its loop: the two are independent refusal
/// reasons a mutation test can target one at a time, and every existing `check_input` call site
/// keeps its two-argument shape.
pub(super) fn check_no_empty_required_wrapper(
    command: &str,
    effect: Effect,
    input: &Value,
) -> Result<(), Refusal> {
    if matches!(effect, Effect::Read | Effect::NotExposed(_)) {
        return Ok(());
    }
    let Some(args) = entry_for(command) else {
        return Ok(());
    };
    let empty = Map::new();
    let given = input.as_object().unwrap_or(&empty);

    for arg in args {
        if !arg.required {
            continue;
        }
        let Some(fields) = arg.fields.filter(|f| !f.is_empty()) else {
            continue;
        };
        let Some(nested) = given.get(arg.name).and_then(Value::as_object) else {
            continue;
        };
        if nested.is_empty() {
            return Err(invalid_input(format!(
                "empty object `{{}}` for required key `{}` on {command} — declared keys under \
                 `{}`: {}",
                arg.name,
                arg.name,
                fields.join(", ")
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
