//! Server-side trust boundary for the Application commands: the input validators
//! and the pure accept/reject core. Split out of `commands/applications.rs` for R8
//! (issue #1280) — every rejection message and byte cap moved verbatim.

use serde_json::Value;

use crate::applications::{ApplicationStore, MAX_JOB_DESCRIPTION_BYTES};
use crate::error::{AppError, AppResult};

/// Server-side trust boundary for the inbound job description. A direct IPC caller
/// bypasses the renderer's Zod cap, so the creation handlers REJECT an oversized
/// value here — up-front, before any store work — against the SAME byte cap the
/// store clamps to ([`MAX_JOB_DESCRIPTION_BYTES`], the single source of truth).
/// The store still clamps as a defense-in-depth second layer. `None` (no
/// description supplied) is always fine.
pub(super) fn reject_oversized_job_description(jd: Option<&str>) -> AppResult<()> {
    if let Some(jd) = jd {
        if jd.len() > MAX_JOB_DESCRIPTION_BYTES {
            return Err(AppError::Validation(format!(
                "job description exceeds the {MAX_JOB_DESCRIPTION_BYTES}-byte limit ({} bytes)",
                jd.len()
            )));
        }
    }
    Ok(())
}

/// Map the inbound `nextActionAt` onto the store's `Option<Option<u64>>` patch shape.
///
/// The field is nullable+optional, so it is generated as `Option<serde_json::Value>`:
/// - absent (`None`) → `Ok(None)` (leave the reminder unchanged);
/// - explicit JSON `null` → `Ok(Some(None))` (clear the reminder);
/// - a `u64` number → `Ok(Some(Some(ms)))` (set it).
///
/// Anything else — a negative, fractional or oversized number, a string, an
/// object — is a caller bug and is REJECTED. It used to be mapped through
/// `Value::as_u64()`, which yields `None` for all of those and therefore produced
/// the same `Some(None)` that means "clear": a caller trying to *set* a bad-typed
/// reminder silently *cleared* it instead, with no error anywhere.
pub(super) fn parse_next_action_at(raw: Option<Value>) -> AppResult<Option<Option<u64>>> {
    match raw {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(other) => other.as_u64().map(|ms| Some(Some(ms))).ok_or_else(|| {
            // Echo the offending JSON type so a renderer sending the wrong shape
            // (a string, a negative/fractional number, an object) sees why.
            let kind = match &other {
                Value::Null => "null",
                Value::Bool(_) => "a boolean",
                Value::Number(_) => "a non-integer or out-of-range number",
                Value::String(_) => "a string",
                Value::Array(_) => "an array",
                Value::Object(_) => "an object",
            };
            AppError::Validation(format!(
                "next_action_at must be null or a non-negative integer epoch-ms, got {kind}"
            ))
        }),
    }
}

/// Validate and normalise an inbound contact email address (the apply-by-email sink).
///
/// Applied to BOTH inbound names for the unified contact pair — `contactEmail`
/// and its deprecated alias `recipientEmail` (see
/// [`crate::applications::Application::recipient_name`]) — since they write the
/// same column; validating only one would leave a bypass under the other name.
///
/// - `None` → `Ok(None)` (field absent — leave unchanged in the store).
/// - Whitespace-only → `Ok(Some(String::new()))` (clear the field).
/// - Otherwise: trim, enforce the 254-byte cap, and check the basic shape
///   (non-empty local part, exactly one `@`, non-empty domain label + TLD with
///   a dot). Returns `Ok(Some(email))` on success, `Err(AppError::Validation)` on
///   invalid input so the renderer can surface a clear reason.
///
/// Every rejection names the field the way the UI labels it — **"contact
/// email"** (`applications.detail.contactEmail`), not the storage/alias
/// identifier `recipient_email`. These strings are rendered verbatim to the user
/// by the inline `role="alert"` on both contact surfaces, and there is no
/// backend i18n catalogue to translate them through, so naming a field the user
/// cannot see made the error unactionable.
pub(super) fn validate_recipient_email(raw: Option<String>) -> AppResult<Option<String>> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() {
        return Ok(Some(String::new())); // whitespace-only → clear
    }
    // Byte-length cap — mirrors the Zod max(254); this is the real trust boundary.
    if trimmed.len() > 254 {
        return Err(AppError::Validation(
            "contact email exceeds the 254-byte limit".into(),
        ));
    }
    // No control characters or spaces ANYWHERE — a bare CR/LF in a stored address
    // is a header-injection primitive for every downstream sink that builds a
    // message from it (the `mailto:` href, any future SMTP send), and a legacy
    // row's unvalidated `contact_email` now mirrors into those sinks under the
    // `recipientEmail` name. Rejected before the shape checks so the error names
    // the real problem. (Interior spaces are only legal in a quoted local part,
    // which this validator has never accepted.)
    if trimmed.chars().any(|c| c.is_control() || c == ' ') {
        return Err(AppError::Validation(
            "contact email must not contain control characters or spaces".into(),
        ));
    }
    // Exactly one '@', non-empty local part, domain must have a dot with a
    // non-empty label on each side of the last dot.
    if trimmed.matches('@').count() != 1 {
        return Err(AppError::Validation(format!(
            "contact email is not a valid address: {trimmed}"
        )));
    }
    let (local, domain) = trimmed.split_once('@').unwrap();
    if local.is_empty() {
        return Err(AppError::Validation(
            "contact email local part must not be empty".into(),
        ));
    }
    let Some(dot) = domain.rfind('.') else {
        return Err(AppError::Validation(format!(
            "contact email domain must contain a dot: {trimmed}"
        )));
    };
    if domain[..dot].is_empty() || domain[dot + 1..].is_empty() {
        return Err(AppError::Validation(format!(
            "contact email has an invalid domain: {trimmed}"
        )));
    }
    Ok(Some(trimmed))
}

/// Server-side cap for the contact NAME, in BYTES. Mirrors the Zod
/// `max(200)` on both inbound names; client validation is UX-only, so this is
/// the real trust boundary (a direct IPC caller bypasses Zod entirely).
pub(super) const MAX_CONTACT_NAME_BYTES: usize = 200;

/// Trim and bound an inbound contact name — the sibling guard to
/// [`validate_recipient_email`], applied to BOTH inbound names (`contactName`
/// and its deprecated alias `recipientName`) since they write the same column.
///
/// - `None` → `Ok(None)` (field absent — leave unchanged).
/// - Whitespace-only → `Ok(Some(String::new()))` (clear the field).
/// - Over the cap → `Err(AppError::Validation)`, never a silent truncation.
/// - Containing a control character → `Err(AppError::Validation)`.
///
/// Interior SPACES are legal here (unlike in an address) — names have them. But
/// control characters are not: this value is interpolated verbatim into the
/// `Apply-by-email: <name> <<email>>` line the contact unification writes into
/// `notes`, and into the display-name half of any future message header, where a
/// bare CR/LF is an injection primitive exactly like the one
/// [`validate_recipient_email`] rejects in the address.
pub(super) fn validate_contact_name(raw: Option<String>) -> AppResult<Option<String>> {
    let Some(s) = raw else {
        return Ok(None);
    };
    let trimmed = s.trim().to_string();
    if trimmed.len() > MAX_CONTACT_NAME_BYTES {
        return Err(AppError::Validation(format!(
            "contact name exceeds the {MAX_CONTACT_NAME_BYTES}-byte limit ({} bytes)",
            trimmed.len()
        )));
    }
    // After the trim, so a surrounding newline is still just whitespace to strip
    // (mirrors the ordering in `validate_recipient_email`).
    if trimmed.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "contact name must not contain control characters".into(),
        ));
    }
    Ok(Some(trimmed))
}

/// Server-side cap for a status-change note, in BYTES. Mirrors the renderer's
/// `NOTE_MAX_LENGTH` (`features/applications/components/StatusNoteModal`); that
/// `maxLength` is a UX guard on one textarea, so THIS is the real bound — a
/// direct IPC caller never passes through it, and the note is appended to the
/// permanent `status_events` history where nothing else bounds it.
pub(super) const MAX_STATUS_NOTE_BYTES: usize = 2_000;

/// Trim and bound an inbound status note.
///
/// REJECTS rather than truncates, consistent with [`validate_contact_name`]:
/// this note is the user's own interaction log, and a silently-halved entry is
/// worse than a visible "too long" they can act on (both surfaces already render
/// the returned `{ error }` inline). `None`/absent and empty are always fine —
/// most transitions carry no note at all.
///
/// Bytes, not chars, matching every other server cap in this module. That is a
/// slightly tighter bound than the client's char-based `maxLength` for
/// multi-byte text; the mismatch surfaces as a clear, recoverable error rather
/// than a silent loss.
pub(super) fn validate_status_note(raw: Option<String>) -> AppResult<String> {
    let trimmed = raw.unwrap_or_default().trim().to_string();
    if trimmed.len() > MAX_STATUS_NOTE_BYTES {
        return Err(AppError::Validation(format!(
            "note exceeds the {MAX_STATUS_NOTE_BYTES}-byte limit ({} bytes)",
            trimmed.len()
        )));
    }
    Ok(trimmed)
}

/// An `id` from the renderer is untrusted IPC input — trim and reject empty
/// before any store work, for BOTH accept/reject commands below. Neither
/// store method can panic on a garbage id (a no-op `Ok(false)`, matched-zero
/// rows), but a rejected-up-front empty id is a clearer signal than a silent
/// "nothing happened" success.
///
/// **MINOR fix: returns the TRIMMED `&str`, not `()`.** This used to
/// validate on `id.trim()` but then discard the trimmed value, so the
/// caller forwarded the ORIGINAL, untrimmed `id` — `" app-1-abcd1234 "`
/// passed this gate (non-empty after trimming) but then matched ZERO rows
/// in both store methods (neither trims), returning `{ "success": true }`
/// — the exact silent "nothing happened" this gate's own doc says it
/// exists to prevent. Forcing the caller to use the RETURNED value (not
/// the original `id` still in scope) makes that class of bug a borrow-
/// checker-shaped mistake to reintroduce, not just a documented intent.
pub(super) fn require_non_empty_id(id: &str) -> AppResult<&str> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "application id is required".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Pure core shared by both accept/reject commands below — testable without
/// a live `AppHandle` (mirrors `extension_bridge::status_update::
/// resolve_status_update`'s factoring). Validates `id`, then delegates to
/// WHICHEVER store method the caller passes as `action` — so the id
/// validation gate is written exactly once, and a test can assert the
/// command-layer path (this fn) produces the SAME store state as calling
/// `action` directly. `event_id` is threaded straight through, unvalidated
/// here (a bogus/foreign id is already a safe no-op at the store layer — see
/// [`crate::applications::ApplicationStore::accept_status_event`]'s doc).
pub(super) fn resolve_status_event_action(
    store: &ApplicationStore,
    id: &str,
    event_id: i64,
    action: impl FnOnce(&ApplicationStore, &str, i64) -> AppResult<bool>,
) -> AppResult<bool> {
    let id = require_non_empty_id(id)?;
    action(store, id, event_id)
}
