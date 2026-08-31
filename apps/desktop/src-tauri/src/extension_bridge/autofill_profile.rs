//! `profile.get` → `profile.result` — assisted-autofill's contact-profile
//! projection. Lifted verbatim out of `mod.rs` (R8 hard-cap relief when
//! adding the origin-gate plumbing for `agent.query` — issue #1084's
//! security-review fixes — pushed that module to the edge) — the exact same
//! reason `msg.rs`/`revoke.rs`/`agent_read.rs` etc. were split out before it.
//! Behaviorally unchanged; only the file it lives in moved.

use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

use super::{msg, BridgeState, AUTOFILL_OFF_MESSAGE};

/// The contact-profile fields sent to the extension for assisted autofill. A
/// flat, string-only projection of [`crate::contact_profile::ContactProfile`]
/// (location collapsed to its default free-text string) — the extension fills
/// matching empty form fields from it and never persists it. Every field is
/// optional (a sparse profile is normal); absent fields are omitted from the wire
/// payload entirely.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AutofillProfile {
    // `pub(super)` (visible throughout `extension_bridge`, not just to
    // descendants of THIS module) — `test`, a SIBLING of this module, reads
    // these fields directly in its own assertions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) linkedin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) github: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) website: Option<String>,
    /// Additional labelled links (Portfolio, Dribbble, Stack Overflow, …) beyond
    /// the named platform fields — see [`clean_extra_links`] for the projection
    /// rules. Additive/optional on the wire: an old extension ignores the key,
    /// and it is omitted entirely (not `[]`) when there is nothing to send, so
    /// an old desktop's replies (which never carry it) parse identically.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) extra_links: Vec<crate::contact_profile::ContactLink>,
}

/// Cap on the number of extra links projected to the extension — a form has no
/// use for an unbounded list, and this bounds the reply size.
pub(super) const MAX_EXTRA_LINKS: usize = 10;

/// Filter + cap the stored extra links for the wire: drop an entry with an empty
/// label, drop a url that (after trimming) is empty or not `http(s)`, then keep
/// at most [`MAX_EXTRA_LINKS`] of what remains, in order. `photo` is never
/// projected at all — unrelated to this list and always dropped.
fn clean_extra_links(
    links: &[crate::contact_profile::ContactLink],
) -> Vec<crate::contact_profile::ContactLink> {
    links
        .iter()
        .filter_map(|link| {
            let label = link.label.trim();
            let url = link.url.trim();
            if label.is_empty() {
                return None;
            }
            let lower = url.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                return None;
            }
            Some(crate::contact_profile::ContactLink {
                label: label.to_string(),
                url: url.to_string(),
            })
        })
        .take(MAX_EXTRA_LINKS)
        .collect()
}

impl AutofillProfile {
    /// Project a stored [`ContactProfile`] to the flat autofill shape. Empty /
    /// whitespace-only values are dropped so the extension never fills a blank.
    pub(super) fn from_contact(p: &crate::contact_profile::ContactProfile) -> Self {
        fn clean(v: &Option<String>) -> Option<String> {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        }
        Self {
            full_name: clean(&p.full_name),
            email: clean(&p.email),
            phone: clean(&p.phone),
            // Collapse the localized location to its default string; the extension
            // fills a single free-text location field.
            location: p
                .location
                .as_ref()
                .map(|l| l.default.trim().to_string())
                .filter(|s| !s.is_empty()),
            linkedin: clean(&p.linkedin),
            github: clean(&p.github),
            website: clean(&p.website),
            extra_links: clean_extra_links(&p.extra_links),
        }
    }
}

/// The opt-in-gated core of a `profile.get`: refuse with a clear, actionable
/// error when autofill is off (never silently return nothing), else project the
/// profile. Pure (no `AppHandle`) so the consent gate is unit-testable.
pub(super) fn resolve_profile(
    enabled: bool,
    profile: Option<&crate::contact_profile::ContactProfile>,
) -> AppResult<AutofillProfile> {
    if !enabled {
        return Err(AppError::Validation(AUTOFILL_OFF_MESSAGE.to_string()));
    }
    let profile =
        profile.ok_or_else(|| AppError::Config("contact profile unavailable".to_string()))?;
    Ok(AutofillProfile::from_contact(profile))
}

/// Build a `profile.result` envelope (success carries the flat profile; refusal /
/// failure carries `error`). Mirrors [`super::import_flow::result_reply`] for the
/// import path.
pub(super) fn profile_result_reply(req_id: &str, outcome: AppResult<AutofillProfile>) -> String {
    let payload = match outcome {
        Ok(p) => serde_json::to_value(&p).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    };
    json!({
        "type": msg::PROFILE_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Read the opt-in + the contact profile off app state and resolve the
/// `profile.get` outcome. Fetch-fresh — nothing is cached; the desktop is the
/// sole owner of the PII. Factored out of [`handle_profile`] so the agent
/// `profile` resource ([`super::agent_read::profile_resource`], a SIBLING
/// module — visible to it with no widening needed) reuses this EXACT consent
/// gate + projection rather than a second profile path.
pub(super) fn profile_outcome(app: &AppHandle) -> AppResult<AutofillProfile> {
    let enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    let profile = app
        .try_state::<crate::contact_profile::ContactProfileStore>()
        .map(|s| s.get());
    resolve_profile(enabled, profile.as_ref())
}

/// Answer an authenticated `profile.get`: return a ready-to-send
/// `profile.result` reply.
pub(super) fn handle_profile(app: &AppHandle, req_id: &str) -> String {
    profile_result_reply(req_id, profile_outcome(app))
}
