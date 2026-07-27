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

use crate::applications::{
    ApplicationMeta, ApplicationStatus, ApplicationStore, MAX_JOB_DESCRIPTION_BYTES,
};
use crate::error::{AppError, AppResult};
use crate::observability::Span;

// Generated from the Zod schemas in packages/shared by `pnpm gen:ipc`.
pub use crate::ipc_contracts::applications::{ApplicationTrackRequest, ApplicationUpdateRequest};

fn store(app: &AppHandle) -> tauri::State<'_, ApplicationStore> {
    app.state::<ApplicationStore>()
}

/// Server-side trust boundary for the inbound job description. A direct IPC caller
/// bypasses the renderer's Zod cap, so the creation handlers REJECT an oversized
/// value here — up-front, before any store work — against the SAME byte cap the
/// store clamps to ([`MAX_JOB_DESCRIPTION_BYTES`], the single source of truth).
/// The store still clamps as a defense-in-depth second layer. `None` (no
/// description supplied) is always fine.
fn reject_oversized_job_description(jd: Option<&str>) -> AppResult<()> {
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
fn parse_next_action_at(raw: Option<Value>) -> AppResult<Option<Option<u64>>> {
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
fn validate_recipient_email(raw: Option<String>) -> AppResult<Option<String>> {
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
            "recipient_email exceeds the 254-byte limit".into(),
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
            "recipient_email must not contain control characters or spaces".into(),
        ));
    }
    // Exactly one '@', non-empty local part, domain must have a dot with a
    // non-empty label on each side of the last dot.
    if trimmed.matches('@').count() != 1 {
        return Err(AppError::Validation(format!(
            "recipient_email is not a valid address: {trimmed}"
        )));
    }
    let (local, domain) = trimmed.split_once('@').unwrap();
    if local.is_empty() {
        return Err(AppError::Validation(
            "recipient_email local part must not be empty".into(),
        ));
    }
    let Some(dot) = domain.rfind('.') else {
        return Err(AppError::Validation(format!(
            "recipient_email domain must contain a dot: {trimmed}"
        )));
    };
    if domain[..dot].is_empty() || domain[dot + 1..].is_empty() {
        return Err(AppError::Validation(format!(
            "recipient_email has an invalid domain: {trimmed}"
        )));
    }
    Ok(Some(trimmed))
}

/// Server-side cap for the contact NAME, in BYTES. Mirrors the Zod
/// `max(200)` on both inbound names; client validation is UX-only, so this is
/// the real trust boundary (a direct IPC caller bypasses Zod entirely).
const MAX_CONTACT_NAME_BYTES: usize = 200;

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
fn validate_contact_name(raw: Option<String>) -> AppResult<Option<String>> {
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
    let note = note.unwrap_or_default();
    match store(&app).set_status(&id, to, &note) {
        Ok(()) => {
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
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
        Err(e) => {
            span.end_with(&e.to_string(), false);
            return json!({ "error": e });
        }
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
        (Err(e), _) | (_, Err(e)) => {
            span.end_with(&e.to_string(), false);
            return json!({ "error": e });
        }
    };
    // Trim + byte-cap both name aliases; whitespace-only collapses to empty
    // (clear the field). Same guard for both, for the same reason as the email.
    let (contact_name, recipient_name) = match (
        validate_contact_name(req.contact_name),
        validate_contact_name(req.recipient_name),
    ) {
        (Ok(c), Ok(r)) => (c, r),
        (Err(e), _) | (_, Err(e)) => {
            span.end_with(&e.to_string(), false);
            return json!({ "error": e });
        }
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
    match result {
        Ok(()) => {
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
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
                log::warn!("[applications] failed to delete child generations (non-fatal): {e}");
            }
        }
    } else if let Some(gens) = app.try_state::<crate::ai_generations::AiGenerationStore>() {
        // Keep documents: detach them so they survive as orphaned generations.
        if let Err(e) = gens.detach_application(&id) {
            log::warn!("[applications] failed to detach child generations (non-fatal): {e}");
        }
    }
    match s.delete(&id, keep_documents) {
        Ok(()) => {
            span.end(true);
            json!({ "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[tauri::command]
pub async fn applications_track(app: AppHandle, req: ApplicationTrackRequest) -> Value {
    let span = Span::begin("applications", "track (manual)".to_string());
    if let Err(e) = reject_oversized_job_description(req.job_description.as_deref()) {
        span.end_with(&e.to_string(), false);
        return json!({ "error": e });
    }
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
    let job_url = req.job_url.unwrap_or_default();
    let board = req.board.unwrap_or_default();
    match store(&app).track_manual(&job_url, &board, &meta) {
        Ok(id) => {
            span.end(true);
            json!({ "id": id, "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[tauri::command]
pub async fn applications_save_from_posting(app: AppHandle, req: ApplicationTrackRequest) -> Value {
    // Jobs-page "Save" → a `saved` (pre-apply) Application. Same request shape as
    // `track`, but the origin keeps it pre-apply instead of marking it applied.
    let span = Span::begin("applications", "save_from_posting".to_string());
    if let Err(e) = reject_oversized_job_description(req.job_description.as_deref()) {
        span.end_with(&e.to_string(), false);
        return json!({ "error": e });
    }
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
    let job_url = req.job_url.unwrap_or_default();
    let board = req.board.unwrap_or_default();
    match store(&app).upsert_for_origin(
        &job_url,
        &board,
        &meta,
        crate::applications::ApplicationOrigin::Saved,
        Some(false),
    ) {
        Ok(id) => {
            span.end(true);
            json!({ "id": id, "success": true })
        }
        Err(e) => {
            span.end_with(&e.to_string(), false);
            json!({ "error": e })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_next_action_at_maps_absent_null_and_valid_numbers() {
        // Absent → leave the stored reminder untouched.
        assert_eq!(parse_next_action_at(None).unwrap(), None);
        // Explicit null → clear it.
        assert_eq!(parse_next_action_at(Some(Value::Null)).unwrap(), Some(None));
        // A real timestamp → set it. Zero is a legitimate epoch value.
        assert_eq!(
            parse_next_action_at(Some(json!(1_767_225_600_000u64))).unwrap(),
            Some(Some(1_767_225_600_000))
        );
        assert_eq!(parse_next_action_at(Some(json!(0))).unwrap(), Some(Some(0)));
    }

    #[test]
    fn parse_next_action_at_rejects_a_non_u64_instead_of_clearing() {
        // `Value::as_u64()` returns None for every one of these, which used to be
        // indistinguishable from an explicit null — so a caller trying to SET a
        // bad-typed reminder silently CLEARED it. They must be errors, not clears.
        for bad in [
            json!(-1),
            json!(1.5),
            json!(u64::MAX as f64 * 2.0),
            json!("1767225600000"),
            json!({}),
            json!([]),
        ] {
            let err = parse_next_action_at(Some(bad.clone()))
                .expect_err(&format!("{bad} must be rejected, not treated as a clear"));
            assert!(
                matches!(err, AppError::Validation(_)),
                "{bad} must fail validation, got {err:?}"
            );
        }
    }

    #[test]
    fn none_description_is_accepted() {
        // No description supplied → always fine (the common path).
        assert!(reject_oversized_job_description(None).is_ok());
    }

    #[test]
    fn under_and_at_cap_are_accepted() {
        // Empty, small, and exactly-at-the-cap inputs all pass — the guard rejects
        // ONLY strictly-oversized input, mirroring the store's `len() <= cap` clamp.
        let at_cap = "a".repeat(MAX_JOB_DESCRIPTION_BYTES);
        for jd in ["", "a normal job ad", at_cap.as_str()] {
            assert!(
                reject_oversized_job_description(Some(jd)).is_ok(),
                "{} bytes must be accepted (cap is {MAX_JOB_DESCRIPTION_BYTES})",
                jd.len()
            );
        }
    }

    #[test]
    fn over_cap_is_rejected() {
        // One byte over the cap → rejected up-front (the direct-IPC abuse path the
        // renderer's Zod cap can't protect). The store still clamps as a second layer.
        let oversized = "a".repeat(MAX_JOB_DESCRIPTION_BYTES + 1);
        let err = reject_oversized_job_description(Some(&oversized))
            .expect_err("an over-cap description must be rejected");
        // It is a typed Validation error (R6), and the message names the byte limit
        // so the renderer can surface a useful reason.
        assert!(
            matches!(err, AppError::Validation(_)),
            "must be a Validation error, got {err:?}"
        );
        assert!(
            err.to_string()
                .contains(&MAX_JOB_DESCRIPTION_BYTES.to_string()),
            "rejection message must mention the byte cap, got {err:?}"
        );
    }

    #[test]
    fn multi_byte_utf8_over_cap_is_rejected() {
        // '€' is 3 UTF-8 bytes. 66_667 repetitions → 200_001 bytes, one over the
        // 200_000-byte cap. A future switch from str.len() (bytes) to char-count
        // would keep 66_667 chars in and this test would catch the regression.
        let s = "€".repeat(66_667);
        assert_eq!(
            s.len(),
            200_001,
            "fixture sanity: expected 200_001 bytes, got {}",
            s.len()
        );
        let err = reject_oversized_job_description(Some(&s))
            .expect_err("multi-byte string over the byte cap must be rejected");
        assert!(
            matches!(err, AppError::Validation(_)),
            "must be a Validation error, got {err:?}"
        );
    }

    #[test]
    fn multi_byte_utf8_at_cap_is_accepted() {
        // '€' is 3 UTF-8 bytes. 66_666 repetitions → 199_998 bytes, within the
        // 200_000-byte cap. Verifies the guard accepts multi-byte content that fits.
        let s = "€".repeat(66_666);
        assert_eq!(
            s.len(),
            199_998,
            "fixture sanity: expected 199_998 bytes, got {}",
            s.len()
        );
        assert!(
            reject_oversized_job_description(Some(&s)).is_ok(),
            "{} bytes must be accepted (cap is {MAX_JOB_DESCRIPTION_BYTES})",
            s.len()
        );
    }

    // ── recipient_email validation ────────────────────────────────────────────

    #[test]
    fn recipient_email_absent_is_passthrough() {
        // None = field not supplied → no update, no error.
        assert!(validate_recipient_email(None).unwrap().is_none());
    }

    #[test]
    fn recipient_email_whitespace_only_clears_field() {
        // Whitespace-only → treat as "clear the field" (not an error).
        let result = validate_recipient_email(Some("   ".into())).unwrap();
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn recipient_email_valid_addresses_accepted() {
        for addr in [
            "user@example.com",
            "first.last@sub.domain.org",
            "user+tag@example.co.uk",
        ] {
            let result = validate_recipient_email(Some(addr.into()));
            assert!(
                result.is_ok(),
                "valid address {addr:?} must be accepted, got {result:?}"
            );
            assert_eq!(result.unwrap(), Some(addr.to_string()));
        }
    }

    #[test]
    fn recipient_email_trims_surrounding_whitespace() {
        let result = validate_recipient_email(Some("  user@example.com  ".into()))
            .unwrap()
            .unwrap();
        assert_eq!(result, "user@example.com");
    }

    #[test]
    fn recipient_email_missing_at_is_rejected() {
        let err = validate_recipient_email(Some("notanemail".into()))
            .expect_err("missing @ must be rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn recipient_email_multiple_at_signs_rejected() {
        let err = validate_recipient_email(Some("a@b@c.com".into()))
            .expect_err("multiple @ must be rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn recipient_email_empty_local_part_rejected() {
        let err = validate_recipient_email(Some("@example.com".into()))
            .expect_err("empty local part must be rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn recipient_email_domain_without_dot_rejected() {
        let err = validate_recipient_email(Some("user@nodot".into()))
            .expect_err("domain without dot must be rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn recipient_email_trailing_dot_in_domain_rejected() {
        // TLD would be empty: "user@example."
        let err = validate_recipient_email(Some("user@example.".into()))
            .expect_err("trailing dot (empty TLD) must be rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn recipient_email_control_characters_and_spaces_fail_validation() {
        // A stored CR/LF is a header-injection primitive for every sink built
        // from this address (the mailto: href today). Reject, never store.
        for bad in [
            "user@example.com\r\nBcc: attacker@evil.test",
            "user@example.com\nBcc: attacker@evil.test",
            "us\ter@example.com",
            "user\u{0000}@example.com",
            "user\u{000B}@example.com",
            // Interior space: never legal in the unquoted local part this
            // validator accepts, and a separator in most address parsers.
            "user name@example.com",
            "user@exa mple.com",
        ] {
            let err = validate_recipient_email(Some(bad.into()))
                .expect_err(&format!("{bad:?} must be rejected"));
            assert!(
                matches!(err, AppError::Validation(_)),
                "{bad:?} must fail validation, got {err:?}"
            );
        }
    }

    #[test]
    fn recipient_email_surrounding_whitespace_still_only_trims() {
        // The control-char guard runs AFTER the trim, so a leading/trailing
        // newline is still just whitespace to strip — not a rejection.
        let ok = validate_recipient_email(Some("\r\n  user@example.com \t".into()))
            .expect("surrounding whitespace is trimmed, not rejected")
            .unwrap();
        assert_eq!(ok, "user@example.com");
    }

    // ── contact name validation ───────────────────────────────────────────────

    #[test]
    fn contact_name_absent_is_passthrough_and_whitespace_clears() {
        assert!(validate_contact_name(None).unwrap().is_none());
        assert_eq!(
            validate_contact_name(Some("   ".into())).unwrap(),
            Some(String::new())
        );
    }

    #[test]
    fn contact_name_is_trimmed_and_bounded() {
        assert_eq!(
            validate_contact_name(Some("  Rita Recruiter  ".into())).unwrap(),
            Some("Rita Recruiter".to_string())
        );
        // Exactly at the cap is accepted; one byte over is rejected (never
        // silently truncated).
        let at_cap = "a".repeat(MAX_CONTACT_NAME_BYTES);
        assert_eq!(
            validate_contact_name(Some(at_cap.clone())).unwrap(),
            Some(at_cap)
        );
        let err = validate_contact_name(Some("a".repeat(MAX_CONTACT_NAME_BYTES + 1)))
            .expect_err("an over-cap name must be rejected");
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[test]
    fn contact_name_control_characters_are_rejected_but_spaces_are_kept() {
        // The name is interpolated verbatim into the migration's
        // "Apply-by-email: <name> <<email>>" note and into the display-name half
        // of any future message header, so a bare CR/LF is the same injection
        // primitive the address guard rejects.
        for bad in [
            "Rita\r\nBcc: attacker@evil.test",
            "Rita\nRecruiter",
            "Rita\tRecruiter",
            "Rita\u{0000}",
        ] {
            let err = validate_contact_name(Some(bad.into()))
                .expect_err(&format!("{bad:?} must be rejected"));
            assert!(
                matches!(err, AppError::Validation(_)),
                "{bad:?} must fail validation, got {err:?}"
            );
        }
        // Interior SPACES are legal in a name (unlike in an address) — real
        // names have them, so the guard must not borrow the email rule wholesale.
        assert_eq!(
            validate_contact_name(Some("Rita von der Recruiter".into())).unwrap(),
            Some("Rita von der Recruiter".to_string())
        );
    }

    #[test]
    fn contact_name_cap_counts_bytes_not_chars() {
        // 'ü' is 2 UTF-8 bytes: 101 chars = 202 bytes > the 200-byte cap. A
        // char-count cap would let this through and the store would keep a
        // longer value than the contract promises.
        let s = "ü".repeat(101);
        assert!(s.chars().count() < MAX_CONTACT_NAME_BYTES);
        assert!(s.len() > MAX_CONTACT_NAME_BYTES);
        assert!(validate_contact_name(Some(s)).is_err());
    }

    #[test]
    fn recipient_email_multibyte_over_254_bytes_rejected() {
        // 'ü' is 2 UTF-8 bytes; 128 repetitions = 256 bytes > 254-byte cap.
        let long = format!("{}@example.com", "ü".repeat(128));
        assert!(long.len() > 254, "fixture must exceed 254 bytes");
        let err = validate_recipient_email(Some(long))
            .expect_err("over-254-byte address must be rejected");
        assert!(matches!(err, AppError::Validation(_)));
    }
}
