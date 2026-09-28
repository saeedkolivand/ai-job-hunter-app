//! Post-persist event + Notification Center push for `import.request` — split from
//! `import_flow.rs` (R8 relief; pure code motion, no behaviour change). See that module's own
//! doc for the whole import flow.

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::events::{emit_event, APPLICATIONS_CHANGED};

/// Tell the renderer to refresh (Applications + Jobs views) and surface a live toast, then drop a
/// Notification Center record. A partial stub has an empty title — fall back to the company
/// (host) so the event payload and toast still name something the user recognizes. The
/// Notification Center push is best-effort and additive: the lists still refresh via the
/// `applications:changed` emit; this only adds the inbox entry + a focused-window toast (OS
/// banner only when unfocused — the import UX intent), routed to the Applications view and
/// highlighting the just-imported row.
pub(super) fn notify_import_result(
    app: &AppHandle,
    id: &str,
    status: &str,
    posting: &crate::scraping::types::JobPosting,
) {
    let title_is_blank = posting.title.trim().is_empty();
    let display_name = if title_is_blank {
        posting.company.clone()
    } else {
        posting.title.clone()
    };
    let body = if title_is_blank {
        posting.company.clone()
    } else {
        format!("{} · {}", posting.title, posting.company)
    };

    emit_event(
        app,
        APPLICATIONS_CHANGED,
        json!({
            "applicationId": id,
            "title": display_name.clone(),
            "company": posting.company.clone(),
            "status": status,
        }),
    );

    let mut search = serde_json::Map::new();
    search.insert("highlight".to_string(), Value::String(id.to_string()));
    crate::commands::notifications::push_and_notify(
        app,
        crate::notifications::NewNotification {
            kind: "import.result".to_string(),
            title: format!("Imported {display_name}"),
            body,
            route: Some(crate::notifications::NotificationRoute {
                to: "/applications".to_string(),
                search: Some(search),
            }),
        },
        crate::commands::notifications::OsBanner::WhenUnfocused,
    );
}
