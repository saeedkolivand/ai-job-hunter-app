//! Application tracking commands — the IPC surface over [`crate::applications`].
//!
//! Seven capabilities (ADR 0001): list/get/set_status/update/delete plus the two
//! creation triggers that are NOT a generation save — `track` (manual create) and
//! `save_from_posting` (Jobs-page Save → `saved`). The Generate trigger lives in
//! [`crate::commands::ai_generations`] (it upserts the Application as a side-effect
//! of saving the document).
//!
//! All handlers use the centralized `AppResult`/`AppError` (serialized to the
//! existing string wire format) and open a trace [`Span`] for the mutating calls.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::applications::{ApplicationMeta, ApplicationStatus, ApplicationStore};
use crate::error::{AppError, AppResult};
use crate::observability::{sanitize_reason, Span};

mod validation;

use validation::{
    parse_next_action_at, reject_oversized_job_description, resolve_status_event_action,
    validate_contact_name, validate_recipient_email, validate_status_note,
};

// Generated from the Zod schemas in packages/shared by `pnpm gen:ipc`.
pub use crate::ipc_contracts::applications::{ApplicationTrackRequest, ApplicationUpdateRequest};

fn store(app: &AppHandle) -> tauri::State<'_, ApplicationStore> {
    app.state::<ApplicationStore>()
}

/// Close `span` as failed and shape the IPC reply (`{ "error": e }`). The error
/// text goes to the span; the typed error itself goes to the renderer.
fn fail(span: &Span, e: AppError) -> Value {
    span.end_with(&e.to_string(), false);
    json!({ "error": e })
}

/// Close `span` for `result`: a success ends it ok and returns the handler's own
/// reply, a failure goes through [`fail`].
fn finish(span: &Span, result: AppResult<Value>) -> Value {
    match result {
        Ok(reply) => {
            span.end(true);
            reply
        }
        Err(e) => fail(span, e),
    }
}

/// The plain `{ "success": true }` reply, for `.map` over any store result.
fn ok_reply<T>(_: T) -> Value {
    json!({ "success": true })
}

/// Split a creation request into the `ApplicationMeta` the store persists and the
/// `(job_url, board)` it keys on. Shared by both creation triggers (`track`,
/// `save_from_posting`), which differ only in the origin they write.
fn creation_target(req: ApplicationTrackRequest) -> (ApplicationMeta, String, String) {
    let meta = ApplicationMeta {
        company: req.company.unwrap_or_default(),
        title: req.title.unwrap_or_default(),
        candidate: req.candidate.unwrap_or_default(),
        brief: String::new(),
        // Carry the posting's description (e.g. an aggregator job whose redirect
        // URL can't be re-resolved) so tailoring has the ad text without a refetch.
        job_description: req.job_description.unwrap_or_default(),
        answers: vec![],
        job_summary: String::new(),
        salary_min: req.salary_min,
        salary_max: req.salary_max,
        salary_currency: req.salary_currency,
    };
    (
        meta,
        req.job_url.unwrap_or_default(),
        req.board.unwrap_or_default(),
    )
}

#[tauri::command]
pub async fn applications_list(app: AppHandle) -> Value {
    serde_json::to_value(store(&app).list()).unwrap_or(json!([]))
}

#[tauri::command]
pub async fn applications_get(app: AppHandle, id: String) -> Value {
    let s = store(&app);
    let app_rec = s.get(&id);
    let events = app_rec.as_ref().map(|_| s.events(&id)).unwrap_or_default();
    json!({ "application": app_rec, "events": events })
}

#[tauri::command]
pub async fn applications_set_status(
    app: AppHandle,
    id: String,
    status: String,
    note: Option<String>,
) -> Value {
    let span = Span::begin("applications", format!("set_status id={id} to={status}"));
    let to = ApplicationStatus::from_id(&status);
    // The note lands in the append-only history, so bound it here — before any
    // store work — rather than trusting the textarea's `maxLength`.
    let note = match validate_status_note(note) {
        Ok(note) => note,
        Err(e) => return fail(&span, e),
    };
    finish(&span, store(&app).set_status(&id, to, &note).map(ok_reply))
}

/// v2 slice 3 (HIGH-1 fix): accept the SPECIFIC email-derived, unconfirmed
/// status-event row `event_id` names — sets its `confirmed` flag to `true` in
/// place, never touching `applications.status` itself (the auto-write
/// already applied it). `event_id` is the [`crate::applications::
/// StatusEvent::event_id`] of the exact row the renderer's Accept button
/// was clicked on — NOT "whichever unconfirmed row is newest": with two
/// provisional rows on the same application (an ordinary sequence — a
/// confirmation email followed by a later rejection email, both still
/// unreviewed), resolving by recency let a click on the OLDER row silently
/// confirm the NEWER, unrelated one instead. A no-op (`{ "success": true }`,
/// nothing to accept — including `event_id` not matching a pending
/// email-derived row for `id` at all) is NOT an error — mirrors
/// [`crate::applications::ApplicationStore::accept_status_event`]'s own
/// contract.
#[tauri::command]
pub async fn applications_accept_status_event(app: AppHandle, id: String, event_id: i64) -> Value {
    let span = Span::begin("applications", format!("accept_status_event id={id}"));
    let result = resolve_status_event_action(
        &store(&app),
        &id,
        event_id,
        ApplicationStore::accept_status_event,
    );
    finish(&span, result.map(ok_reply))
}

/// v2 slice 3 (HIGH-1 fix): reject the SPECIFIC email-derived, unconfirmed
/// status-event row `event_id` names — reverts the status BY
/// COMPARE-AND-SET (a status the user changed by hand in the meantime is
/// never clobbered; the provisional row is simply dismissed instead) and
/// appends a reversal event. Append-only: the original transition row is
/// never edited or deleted. Same `event_id`-targeting rationale as
/// [`applications_accept_status_event`] above — see
/// [`crate::applications::ApplicationStore::reject_status_event`].
#[tauri::command]
pub async fn applications_reject_status_event(app: AppHandle, id: String, event_id: i64) -> Value {
    let span = Span::begin("applications", format!("reject_status_event id={id}"));
    let result = resolve_status_event_action(
        &store(&app),
        &id,
        event_id,
        ApplicationStore::reject_status_event,
    );
    finish(&span, result.map(ok_reply))
}

/// Patch the user-editable tracking fields of one Application.
///
/// **Contact fields converge:** `contactName`/`contactEmail` are canonical and
/// `recipientName`/`recipientEmail` are accepted deprecated aliases of them — a
/// write under either name lands in the same storage, and every response carries
/// both names populated with that single value. When a caller sends both, the
/// canonical one wins. See [`crate::applications::Application::recipient_name`].
#[tauri::command]
pub async fn applications_update(app: AppHandle, req: ApplicationUpdateRequest) -> Value {
    let span = Span::begin("applications", format!("update id={}", req.id));
    // `nextActionAt` is nullable+optional → generated as `Option<serde_json::Value>`.
    // Absent (None) = leave unchanged; explicit JSON `null` = clear the reminder;
    // a number = set it. Map that to the store's `Option<Option<u64>>` patch shape.
    let next_action_at = match parse_next_action_at(req.next_action_at) {
        Ok(v) => v,
        Err(e) => return fail(&span, e),
    };
    // Server-side contact-email validation: trim, whitespace-only → clear,
    // bad format → Validation error. This is the apply-by-email sink — a bad
    // address must never be stored. Both inbound names hit the same column
    // (contact unification), so both go through the same guard.
    let (contact_email, recipient_email) = match (
        validate_recipient_email(req.contact_email),
        validate_recipient_email(req.recipient_email),
    ) {
        (Ok(c), Ok(r)) => (c, r),
        (Err(e), _) | (_, Err(e)) => return fail(&span, e),
    };
    // Trim + byte-cap both name aliases; whitespace-only collapses to empty
    // (clear the field). Same guard for both, for the same reason as the email.
    let (contact_name, recipient_name) = match (
        validate_contact_name(req.contact_name),
        validate_contact_name(req.recipient_name),
    ) {
        (Ok(c), Ok(r)) => (c, r),
        (Err(e), _) | (_, Err(e)) => return fail(&span, e),
    };
    let result = store(&app).update_fields(
        &req.id,
        req.notes,
        next_action_at,
        req.comp,
        contact_name,
        contact_email,
        req.job_description,
        req.job_summary,
        recipient_name,
        recipient_email,
    );
    finish(&span, result.map(ok_reply))
}

#[tauri::command]
pub async fn applications_delete(app: AppHandle, id: String, keep_documents: bool) -> Value {
    let span = Span::begin(
        "applications",
        format!("delete id={id} keep_documents={keep_documents}"),
    );
    let s = store(&app);
    // When NOT keeping documents, delete the child generations linked to this
    // Application first; either way the Application + its history are removed.
    if !keep_documents {
        if let Some(gens) = app.try_state::<crate::ai_generations::AiGenerationStore>() {
            if let Err(e) = gens.remove_for_application(&id) {
                log::warn!(
                    "[applications] failed to delete child generations (non-fatal): {}",
                    sanitize_reason(&e.to_string())
                );
            }
        }
    } else if let Some(gens) = app.try_state::<crate::ai_generations::AiGenerationStore>() {
        // Keep documents: detach them so they survive as orphaned generations.
        if let Err(e) = gens.detach_application(&id) {
            log::warn!(
                "[applications] failed to detach child generations (non-fatal): {}",
                sanitize_reason(&e.to_string())
            );
        }
    }

    // The posting url of the PIPELINE RUN TRAIL to purge, read BEFORE the
    // delete because the row is the only thing that can answer it — and used
    // only if the delete SUCCEEDS (see the `Ok` arm).
    //
    // Why the trail is in scope at all: a max-depth run persists its full
    // re-seeded strategy (the whole employment history) and its full evidence
    // map (verbatim résumé quotes) in `pipeline_run_events.artifact_json`,
    // deliberately — it is the only copy a per-entry regenerate can read hours
    // later. Nothing else ever removes it (retention only evicts the FOURTH run
    // of a posting still being run) and `DataStore::export` ships every event
    // row into the user's backups.
    //
    // Only on the delete-everything arm: with `keep_documents` the trail is no
    // more sensitive than the `ai_generations` row being kept on purpose, and it
    // is what makes the kept document's own runs panel readable.
    let job_url = (!keep_documents)
        .then(|| s.get(&id).map(|application| application.job_url))
        .flatten();

    match s.delete(&id, keep_documents) {
        Ok(()) => {
            // AFTER the parent delete committed, never before. The trail is the
            // one child here that cannot be reconstructed, and a `SQLITE_BUSY`
            // or IO failure on `s.delete` would otherwise leave the application
            // alive with its history already irreversibly gone — the user sees
            // an error, retries, and the run trail they never asked to lose is
            // simply absent. `ai_generations_remove`'s cascade refuses the same
            // ordering for the same reason.
            if let (Some(job_url), Some(runs)) = (
                job_url,
                app.try_state::<crate::pipeline::runs::PipelineRunStore>(),
            ) {
                // …unless a GENERATION still owns that posting. `ai_generations`
                // has a unique partial index on `job_url`, so at most one row
                // can, and `remove_for_application` above only deleted the rows
                // still LINKED to this application — a generation detached by
                // an earlier `keep_documents` delete survives on purpose and
                // keeps the same url. Purging then takes the trail of a
                // document the user explicitly chose to keep: its runs panel
                // empties and per-entry regenerate loses the artifacts
                // `artifacts_for` reads.
                let still_owned = app
                    .try_state::<crate::ai_generations::AiGenerationStore>()
                    .and_then(|gens| gens.find_for_job(&job_url))
                    .is_some();
                if !still_owned {
                    runs.delete_for_job(&job_url);
                }
            }
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => fail(&span, e),
    }
}

#[tauri::command]
pub async fn applications_track(app: AppHandle, req: ApplicationTrackRequest) -> Value {
    let span = Span::begin("applications", "track (manual)".to_string());
    if let Err(e) = reject_oversized_job_description(req.job_description.as_deref()) {
        return fail(&span, e);
    }
    let (meta, job_url, board) = creation_target(req);
    let result = store(&app).track_manual(&job_url, &board, &meta);
    finish(&span, result.map(|id| json!({ "id": id, "success": true })))
}

#[tauri::command]
pub async fn applications_save_from_posting(app: AppHandle, req: ApplicationTrackRequest) -> Value {
    // Jobs-page "Save" → a `saved` (pre-apply) Application. Same request shape as
    // `track`, but the origin keeps it pre-apply instead of marking it applied.
    let span = Span::begin("applications", "save_from_posting".to_string());
    if let Err(e) = reject_oversized_job_description(req.job_description.as_deref()) {
        return fail(&span, e);
    }
    let (meta, job_url, board) = creation_target(req);
    let result = store(&app).upsert_for_origin(
        &job_url,
        &board,
        &meta,
        crate::applications::ApplicationOrigin::Saved,
        Some(false),
    );
    finish(&span, result.map(|id| json!({ "id": id, "success": true })))
}

#[cfg(test)]
mod tests;
