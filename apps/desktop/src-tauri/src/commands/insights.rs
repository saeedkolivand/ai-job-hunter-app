//! Outcome insights + volume nudge: the read-only IPC surface over
//! [`crate::insights`]. Derived on every call from the application and
//! generation stores; nothing is stored (ADR-042).

use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::ai_generations::AiGenerationStore;
use crate::applications::ApplicationStore;
use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::insights::{build_inputs, report, InsightsReport};

/// Group every sent application by outcome (board, tailored, cover letter,
/// target language, month, template) and report the volume nudge when it fires.
/// Runs on a blocking thread: it reads both stores in full.
#[tauri::command]
pub async fn insights_outcomes(app: AppHandle) -> AppResult<InsightsReport> {
    tauri::async_runtime::spawn_blocking(move || {
        let applications = app
            .try_state::<ApplicationStore>()
            .ok_or_else(|| AppError::Message("applications store unavailable".into()))?;
        let generations = app
            .try_state::<AiGenerationStore>()
            .ok_or_else(|| AppError::Message("generations store unavailable".into()))?;
        let now = now_ms();
        // One consistent read; a store failure fails the report rather than yielding
        // an empty (or history-less) one.
        let mut apps = Vec::new();
        let mut histories = HashMap::new();
        for (application, history) in applications.list_with_events()? {
            histories.insert(application.id.clone(), history);
            apps.push(application);
        }
        let unreadable = histories.values().filter(|h| h.is_none()).count();
        if unreadable > 0 {
            log::warn!(
                "insights: {unreadable} status histories unreadable, excluded from outcomes"
            );
        }
        let inputs = build_inputs(
            &apps,
            &generations.list(),
            |id| histories.get(id).cloned().flatten(),
            now,
        );
        Ok(report(&inputs, now))
    })
    .await
    .map_err(|e| AppError::Message(format!("insights task failed: {e}")))?
}
