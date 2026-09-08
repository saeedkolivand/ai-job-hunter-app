//! ADR-038 §4, Phase 3 — resolving an [`Effect::Irreversible`] row's
//! `--confirm` value. Split out of `agent_call.rs` to keep that file under
//! R8's LOC cap (the same reason `documents/sql.rs`/`applications/reminders.rs`
//! exist) — this is real logic, not tests, so it earns its own file rather
//! than living in `agent_call/tests.rs`.
//!
//! Every fn here is split pure/impure: [`resolve`] is the ONLY one that
//! touches [`AppHandle`] — it dispatches `source.read_command()` through
//! [`super::invoke_command`], the SAME real command body every other row
//! already uses, never a second implementation of that command's logic.
//! [`extract`]/[`build_input`]/[`hint`] are pure `Value`-in,
//! `Value`/`String`-out — directly unit-testable with hand-built fixtures,
//! no live app, mirroring this crate's standing pure-core/impure-shell split
//! (`agent_read::resolve_job`/`job_resource`, `resolve_best_matches`/
//! `best_matches_resource`).

use serde_json::Value;
use tauri::AppHandle;

use super::super::agent_cli::policy::{LookupInput, ProofSource, POLICY};

/// The input body `source.read_command()` is invoked with — pure, so a
/// caller-controlled `caller_input` can never smuggle anything past this
/// beyond the ONE key a [`ProofSource::Lookup`] row explicitly forwards.
/// `LookupInput::FromCaller` is a PATH into `caller_input` (walked via
/// [`walk`], the SAME fn a response path uses below) rather than a flat
/// top-level field — see `LookupInput::FromCaller`'s own doc for why a flat
/// field silently read the wrong location for a command whose
/// `#[tauri::command]` signature wraps its args in one `req` struct.
fn build_input(source: ProofSource, caller_input: &Value) -> Value {
    match source {
        ProofSource::Lookup { key, input, .. } => {
            let value = match input {
                LookupInput::Literal(v) => Value::String(v.to_string()),
                LookupInput::FromCaller(path) => {
                    walk(caller_input, path).cloned().unwrap_or(Value::Null)
                }
            };
            serde_json::json!({ key: value })
        }
        ProofSource::Scalar { .. }
        | ProofSource::ListMatch { .. }
        | ProofSource::Count { .. }
        | ProofSource::MatchCount { .. } => serde_json::json!({}),
    }
}

/// Walk `path` (a sequence of object keys) into `value`; an empty `path`
/// returns `value` itself (e.g. `system_get_version`'s bare string response).
/// ONE walker for BOTH directions this module reads a path from (HIGH fix —
/// security review round 2): a `ProofSource`'s own response `path`
/// ([`ProofSource::Scalar`]/[`ProofSource::Lookup`]) AND a
/// [`LookupInput::FromCaller`]/`ProofSource::ListMatch`'s `id_field`/
/// `ProofSource::MatchCount`'s `ids_field` path into the CALLER's `--input`
/// — never two copies of the same walk that could silently diverge.
fn walk<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, key| v.get(key))
}

/// Render a resolved [`Value`] as the exact string a caller's `--confirm`
/// must match — only scalar shapes are ever a valid proof; an object/array/
/// null is a resolution failure (nothing to confirm against), never
/// stringified as `"null"`/`"{}"` (which would make an ABSENT record
/// satisfiable by typing a literal word).
fn stringify(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// The pure extraction core: given the ALREADY-FETCHED `read_command`
/// response and the irreversible command's own `caller_input`, compute the
/// expected proof string — or `None` if the target record cannot be
/// resolved (e.g. deleting a document whose id no longer exists, or a
/// caller who omitted the id field the ceremony needs to look it up).
pub(super) fn extract(
    source: ProofSource,
    caller_input: &Value,
    response: &Value,
) -> Option<String> {
    match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
            stringify(walk(response, path)?)
        }
        ProofSource::ListMatch {
            id_field,
            match_field,
            value_field,
            ..
        } => {
            let target = walk(caller_input, id_field)?;
            let record = response
                .as_array()?
                .iter()
                .find(|record| record.get(match_field) == Some(target))?;
            stringify(record.get(value_field)?)
        }
        ProofSource::Count { .. } => Some(response.as_array()?.len().to_string()),
        ProofSource::MatchCount {
            ids_field,
            match_field,
            ..
        } => {
            let ids = walk(caller_input, ids_field)?.as_array()?;
            let count = response
                .as_array()?
                .iter()
                .filter(|record| record.get(match_field).is_some_and(|id| ids.contains(id)))
                .count();
            Some(count.to_string())
        }
    }
}

/// Fence `response` the SAME way [`super::dispatch_direct`] fences every
/// other response this dispatcher hands to a caller, then [`extract`] —
/// split out of [`resolve`] as its own pure fn (HIGH fix — security review
/// round 4) so this composition is directly unit-testable without an
/// `AppHandle`, mirroring every other pure/impure split in this file. Before
/// this fix, `resolve` extracted from the RAW response, while every read a
/// caller could actually run to learn the same value went through
/// `dispatch_direct` first, which fences `title`/`company`/`location`/etc
/// (`FENCE_FIELD_NAMES`). A confirm ceremony whose proof field is one of
/// those names was permanently unsatisfiable: the caller only ever sees the
/// FENCED string (`<job_posting>...\n</job_posting>`), but `--confirm` was
/// checked against the RAW one — `applications_delete`'s `title` proof and
/// `notifications_remove`'s `title` proof both hit this the moment `title`
/// joined the fence list. Fencing here too makes both sides agree: the value
/// a caller reads through this dispatcher and the value `--confirm` is
/// checked against are now the exact same transform of the exact same read,
/// never two different views of one record.
fn extract_from_fenced_response(
    source: ProofSource,
    caller_input: &Value,
    mut response: Value,
) -> Option<String> {
    super::fence_scraped_fields(&mut response);
    // A3-r1-AC-6: mirror `reshape_reply`'s SECOND fencing step too, not only the first — a
    // `read_command` whose whole reply is a bare user-document string (e.g. `documents_get_text`)
    // is fenced by `fence_user_document_bare_text`, never by `fence_scraped_fields` (named-field
    // only). Without this, a proof bound to such a command would compare the raw value against
    // the fenced one a caller actually reads. No real `Irreversible` row proves on one today
    // (verified against every `read_command:` in `policy.rs`) — a latent-bug close, not live.
    super::reshape::fence_user_document_bare_text(source.read_command(), &mut response);
    extract(source, caller_input, &response)
}

/// The impure shell: dispatch `source.read_command()` for real, then
/// [`extract_from_fenced_response`]. `None` on anything that stops this from
/// producing a usable proof — the caller (`dispatch_irreversible`) turns
/// that into [`super::Refusal::ProofUnavailable`], never a panic and never a
/// value this fn invents.
pub(super) async fn resolve(
    app: &AppHandle,
    source: ProofSource,
    caller_input: &Value,
) -> Option<String> {
    let read_input = build_input(source, caller_input);
    let outcome = super::invoke_command(app, source.read_command(), read_input)
        .await
        .ok()?;
    let response = match outcome {
        super::InvokeOutcome::Success(v) => v,
        // The read this proof depends on itself hit a Tauri-level error
        // (HIGH fix — security review: the old fold-into-Ok behaviour meant
        // this used to treat that error VALUE as the resolved proof) — no
        // proof value exists to extract; degrade to `None` like any other
        // resolution failure, never a panic and never a value invented here.
        super::InvokeOutcome::CommandErr(_) => return None,
    };
    extract_from_fenced_response(source, caller_input, response)
}

/// The proof value's own field NAME/PATH (never the value) — `commands`' `proofField` row (issue
/// #1160): "what would deleting this require?" answerable without dispatching. Joined with `.` for
/// a multi-segment `Scalar`/`Lookup` path (e.g. `"application.title"`) — the full path a caller
/// would read a response at, unlike [`hint`]'s own per-variant match, which only needs the LAST
/// segment (to check membership in `super::FENCE_FIELD_NAMES`). `None` for `Count`/`MatchCount`
/// (the proof is a DERIVED number, not a field on the read response — nothing to name) and for a
/// `Scalar`/`Lookup` with an EMPTY path (the proof is the bare response value itself, e.g.
/// `system_get_version`'s — same "its own response value" case [`hint`] spells out in prose).
/// `pub(super)` — re-exported by [`super::proof_field_for`] for `agent_cli::mcp` (this module
/// itself stays private to `agent_call`; see that fn's own doc for why).
pub(super) fn proof_field(source: ProofSource) -> Option<String> {
    match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
            (!path.is_empty()).then(|| path.join("."))
        }
        ProofSource::ListMatch { value_field, .. } => Some(value_field.to_string()),
        ProofSource::Count { .. } | ProofSource::MatchCount { .. } => None,
    }
}

/// What [`proof_field`]'s `None` means for THIS `source` — the discriminator CLI review round 2
/// (MEDIUM) asked for: an absent `proofField` used to conflate two unrelated causes (`Count`/
/// `MatchCount`, where the proof is a DERIVED NUMBER with no field to name at all, and a `Scalar`/
/// `Lookup` with an empty path, where the proof IS the response value, just unnamed) into one
/// "nothing to go on" shape — exactly the `CatalogueArg::fields` null-vs-omitted collapse this
/// same table already went out of its way to avoid. Always returns a value (never `None` itself)
/// so `commands` can carry it on EVERY `Irreversible` row, not only the 26 with a named field.
pub(super) fn proof_kind(source: ProofSource) -> &'static str {
    match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } if path.is_empty() => {
            "response_value"
        }
        ProofSource::Scalar { .. } | ProofSource::Lookup { .. } | ProofSource::ListMatch { .. } => {
            "field"
        }
        ProofSource::Count { .. } | ProofSource::MatchCount { .. } => "count",
    }
}

/// The `ConfirmationRequired` refusal's own detail text — names WHICH read
/// surface and field the proof comes from, NEVER the value (ADR-038 §4's
/// entire point). The namespace prefix is derived from [`POLICY`] itself via
/// [`super::split_path`] (never a second hand-typed mapping), so it can
/// never drift from the table that actually backs it.
pub(super) fn hint(source: ProofSource) -> String {
    let bare = source.read_command();
    let target = POLICY
        .iter()
        .find_map(|entry| {
            let (ns, cmd) = super::split_path(entry.path);
            (cmd == bare).then(|| format!("`agent call {ns}:{cmd}`"))
        })
        .unwrap_or_else(|| format!("`{bare}`"));
    let field = match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
            if path.is_empty() {
                "its own response value".to_string()
            } else {
                format!("its own `{}` field", path.join("."))
            }
        }
        // The three list-shaped sources below all name a read command that
        // MAY be one of `super::PAGINATED_LIST_COMMANDS`
        // (`applications_list` backs `privacy_reset_app`'s Count,
        // `ai_generations_list` backs both `ai_generations_remove` and
        // `ai_generations_remove_bulk`). Those replies are no longer a bare
        // array: issue #1136 wrapped them in `{items,total,nextCursor}`, so a
        // hint that still said "its own array length" pointed a caller at a
        // key that is not there and at a page that may not hold the record.
        // Worded generically rather than per-command — the wording stays
        // correct for an unpaged row too, and a second copy of the paged-row
        // list here would be exactly the drift this module avoids elsewhere.
        ProofSource::ListMatch { value_field, .. } => format!(
            "the matching record's own `{value_field}` field (paging with `cursor` if it is \
             not on the first page)"
        ),
        ProofSource::Count { .. } => {
            "its own array length: how many records exist (a paged reply reports this as \
             `total`)"
                .to_string()
        }
        ProofSource::MatchCount { .. } => {
            "the count of the targeted ids that actually exist (paging with `cursor` if they \
             are not all on the first page)"
                .to_string()
        }
    };
    format!("read {target} and pass {field} as --confirm")
}

// ── Grace window for a proof value disclosed via `confirmation_required` (issue #1162) ──────
//
// `ai_spend_summary`'s `today.inputTokens` backs ten `Irreversible` rows and advances under
// ORDINARY background AI activity between the moment a caller reads it and the moment it presents
// that same value as `--confirm`. Fix: remember the value CURRENT at `confirmation_required` time
// and accept a presented value matching that snapshot for a short window after.
//
// Deliberately narrow (security review round A3-r1, AC-1/SEC-1 CRITICAL): the window applies ONLY
// to [`GRACE_WINDOW_READ_COMMAND`]. Every other row's proof is bound to a caller-chosen target (a
// document id, a run id…) and only an exact match on the FRESH value is ever accepted — no
// snapshot, no window, so a value disclosed for one target can never authorise a different one.
// `ai_spend_summary`'s `Scalar` proof is the one shape with NO per-target caller input at all
// (`build_input` always resolves it with `{}`), so there is no target to confuse in the first
// place. Widening this to every `Scalar` row (`ai_active_config`/`system_get_version`, neither of
// which has a background-drift problem to solve) would reopen that risk for no benefit.

/// The ONE read command whose `Irreversible` rows get a grace window. `Scalar`'s `path` is
/// identical on every real row naming this read command (verified in `policy.rs`), so one shared
/// snapshot is exactly as precise as one per irreversible command name, and lets
/// [`refresh_from_read`] update it from a single place regardless of which of the ten commands the
/// caller is about to confirm.
// `pub(super)` (A3-r2-AC-6) -- `agent_call::tests`' POLICY-scanning regression test names both.
pub(super) const GRACE_WINDOW_READ_COMMAND: &str = "ai_spend_summary";
pub(super) const GRACE_WINDOW_PATH: &[&str] = &["today", "inputTokens"];

/// How long a snapshot stays acceptable even after the CURRENT value has moved. ~120s: generous
/// enough for "read the proof, paste it back", short enough not to become a standing credential.
const PROOF_SNAPSHOT_TTL: std::time::Duration = std::time::Duration::from_secs(120);

/// One remembered proof value. `at` is [`std::time::Instant`] — monotonic, immune to a clock
/// adjustment reviving an expired snapshot.
struct ProofSnapshot {
    value: String,
    at: std::time::Instant,
}

/// Keyed by [`GRACE_WINDOW_READ_COMMAND`] alone (the map holds at most one entry in practice).
static PROOF_SNAPSHOTS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<&'static str, ProofSnapshot>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Whether `source`'s `Irreversible` row gets a grace window at all — `Some(key)` when it does,
/// `None` for every other row, which [`accepted_at`] then never consults the snapshot map for.
pub(super) fn grace_window_key(source: ProofSource) -> Option<&'static str> {
    (source.read_command() == GRACE_WINDOW_READ_COMMAND).then_some(GRACE_WINDOW_READ_COMMAND)
}

/// Record `value` as the current grace-window snapshot — called right after a
/// `confirmation_required` refusal for a [`grace_window_key`]-eligible row, BEFORE the caller
/// could have read this value any other way. Split from [`remember_at`] so a test can drive the
/// pure core against a manufactured `now`.
pub(super) fn remember(key: &'static str, value: String) {
    remember_at(key, value, std::time::Instant::now());
}

fn remember_at(key: &'static str, value: String, now: std::time::Instant) {
    let mut map = PROOF_SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
    map.insert(key, ProofSnapshot { value, at: now });
}

/// Refresh the grace-window snapshot from a response the caller just read DIRECTLY through
/// [`super::dispatch_direct`] (never one `resolve` fetched on the caller's own behalf). Closes the
/// double-drift gap security review round A3-r1's AC-7 flagged: the t0 snapshot and the value the
/// caller actually reads before retrying (t1) can differ if the counter moves in both intervals,
/// so this keeps the snapshot current with whichever value the caller most recently saw. A no-op
/// for every other command.
pub(super) fn refresh_from_read(command: &str, response: &Value) {
    if command != GRACE_WINDOW_READ_COMMAND {
        return;
    }
    if let Some(value) = walk(response, GRACE_WINDOW_PATH).and_then(stringify) {
        remember(GRACE_WINDOW_READ_COMMAND, value);
    }
}

/// Why a presented `--confirm` value was refused when it isn't the FRESH proof (issue #1162):
/// [`Mismatch`] is an ordinary wrong guess; [`Expired`] matches a real, remembered snapshot whose
/// window has closed, telling the caller to re-read rather than "you guessed wrong".
///
/// [`Mismatch`]: SnapshotOutcome::Mismatch
/// [`Expired`]: SnapshotOutcome::Expired
pub(super) enum SnapshotOutcome {
    Mismatch,
    Expired,
}

/// Whether `presented` is acceptable, given the FRESH `current` value and `key` —
/// [`grace_window_key`]'s verdict for the row being confirmed. An exact match on `current` always
/// succeeds first, so this still works for a `None`-key row too. Otherwise only a `Some` key may
/// match the snapshot [`remember`]/[`refresh_from_read`] last recorded for it within
/// [`PROOF_SNAPSHOT_TTL`] — `None` refuses immediately, never touching the map (AC-1/SEC-1
/// CRITICAL: no snapshot can ever authorise a different target's ceremony). A matching snapshot is
/// consumed on accept (SEC-2 HIGH): one disclosure buys exactly one dispatch. Never discloses
/// `current` or the snapshot value — only yes/no.
pub(super) fn accepted(
    key: Option<&'static str>,
    current: &str,
    presented: &str,
) -> Result<(), SnapshotOutcome> {
    accepted_at(key, current, presented, std::time::Instant::now())
}

fn accepted_at(
    key: Option<&'static str>,
    current: &str,
    presented: &str,
    now: std::time::Instant,
) -> Result<(), SnapshotOutcome> {
    if presented == current {
        // A3-r2-AC-3 HIGH -- consume any snapshot for `key` here too, or it survives to
        // authorise a second dispatch once the live counter moves back onto the disclosed value.
        if let Some(key) = key {
            let mut map = PROOF_SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
            map.remove(key);
        }
        return Ok(());
    }
    let Some(key) = key else {
        return Err(SnapshotOutcome::Mismatch);
    };
    let mut map = PROOF_SNAPSHOTS.lock().unwrap_or_else(|e| e.into_inner());
    let outcome = match map.get(key) {
        Some(snap)
            if snap.value == presented
                && now.saturating_duration_since(snap.at) <= PROOF_SNAPSHOT_TTL =>
        {
            Ok(())
        }
        Some(snap) if snap.value == presented => Err(SnapshotOutcome::Expired),
        _ => Err(SnapshotOutcome::Mismatch),
    };
    if outcome.is_ok() {
        map.remove(key);
    }
    outcome
}

/// A3-r2-AC-4: serializes every test touching [`PROOF_SNAPSHOTS`] under the literal
/// [`GRACE_WINDOW_READ_COMMAND`] key -- the one key that isn't test-choosable, so two tests on
/// the real grace-window path race on the shared map without this lock.
#[cfg(test)]
pub(super) static GRACE_WINDOW_KEY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests;
