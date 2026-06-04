use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ipc_contracts::referrals::ReferralUpsertRequest;
use crate::referrals::{make_referral_id, now_ms, ReferralContact, ReferralStore};

#[tauri::command]
pub async fn referrals_list(app: AppHandle, job_url: Option<String>) -> Value {
    let store = app.state::<ReferralStore>();
    let records = match job_url {
        Some(url) if !url.is_empty() => store.list_by_job(&url),
        _ => store.list(),
    };
    serde_json::to_value(records).unwrap_or(json!([]))
}

/// Channels the renderer is allowed to set (matches the contract union).
const ALLOWED_CHANNELS: &[&str] = &["email", "linkedin_message", "connection_note"];
/// Lifecycle statuses the renderer is allowed to set.
const ALLOWED_STATUSES: &[&str] = &["draft", "sent", "replied"];

#[tauri::command]
pub async fn referrals_upsert(app: AppHandle, req: ReferralUpsertRequest) -> Value {
    let store = app.state::<ReferralStore>();

    // Allowlist the free-string enums before persisting verbatim.
    if !ALLOWED_CHANNELS.contains(&req.channel.as_str()) {
        return json!({ "error": "invalid channel" });
    }
    if !ALLOWED_STATUSES.contains(&req.status.as_str()) {
        return json!({ "error": "invalid status" });
    }

    let now = now_ms();

    // An absent id inserts a fresh row (new id + created_at = now); a present id
    // overwrites that row. created_at is always passed as `now`: the DB keeps the
    // existing row's value on conflict (COALESCE in `upsert`), so the bound value
    // only applies on a true insert — no read-then-write race here. updated_at
    // always tracks the latest write.
    let id = match req.id {
        Some(id) if !id.is_empty() => id,
        _ => make_referral_id(),
    };

    let rec = ReferralContact {
        id,
        job_url: req.job_url,
        company_name: req.company_name,
        person_name: req.person_name,
        person_role: req.person_role.unwrap_or_default(),
        linkedin_url: req.linkedin_url.unwrap_or_default(),
        email_draft: req.email_draft.unwrap_or_default(),
        message_draft: req.message_draft.unwrap_or_default(),
        invite_note_draft: req.invite_note_draft.unwrap_or_default(),
        channel: req.channel,
        status: req.status,
        notes: req.notes.unwrap_or_default(),
        created_at: now,
        updated_at: now,
    };

    match store.upsert(&rec) {
        Ok(()) => serde_json::to_value(rec).unwrap_or(json!({ "error": "serialize failed" })),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
pub async fn referrals_remove(app: AppHandle, id: String) -> Value {
    if id.trim().is_empty() {
        return json!({ "error": "invalid id" });
    }
    let store = app.state::<ReferralStore>();
    match store.remove(&id) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}
