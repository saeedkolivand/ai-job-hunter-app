//! Outcome insights + volume nudge: the read-only IPC surface over
//! [`crate::insights`]. Derived on every call from the application and
//! generation stores; nothing is stored (ADR-042).

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
        let inputs = build_inputs(
            &applications.list(),
            &generations.list(),
            |id| applications.events(id),
            now,
        );
        Ok(report(&inputs, now))
    })
    .await
    .map_err(|e| AppError::Message(format!("insights task failed: {e}")))?
}
