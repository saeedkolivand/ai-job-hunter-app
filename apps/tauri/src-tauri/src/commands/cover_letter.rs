use tauri::AppHandle;

use crate::cover_letter::{CoverLetterRequest, CoverLetterResponse};

/// Generate a cover letter entirely server-side.
/// API keys never leave Rust — they are read from the OS keychain.
#[tauri::command]
pub async fn generate_cover_letter(
    app: AppHandle,
    req: CoverLetterRequest,
) -> Result<CoverLetterResponse, String> {
    crate::cover_letter::run_pipeline(app, req).await
}
