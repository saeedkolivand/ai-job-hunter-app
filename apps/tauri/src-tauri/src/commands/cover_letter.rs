use tauri::AppHandle;

use crate::cover_letter::{CoverLetterRequest, CoverLetterResponse};
use crate::error::AppResult;

/// Generate a cover letter entirely server-side.
/// API keys never leave Rust — they are read from the OS keychain.
#[tauri::command]
pub async fn generate_cover_letter(
    app: AppHandle,
    req: CoverLetterRequest,
) -> AppResult<CoverLetterResponse> {
    crate::cover_letter::run_pipeline(app, req).await
}
