//! Résumé extraction IPC surface.
//!
//! Thin shell wrapper over [`crate::extraction`]. The extraction logic itself is
//! Tauri-free; keeping the `#[tauri::command]` here makes the shell layer the sole
//! owner of command definitions (see docs/architecture-rules.md R1).

use crate::error::AppResult;
use crate::extraction::{self, types::ExtractedResume};

/// Extract plain text + structured fields from a résumé file
/// (PDF/DOCX/TXT/RTF/HTML). Routes to the data-driven extractor registry.
#[tauri::command]
pub async fn extract_resume(path: String) -> AppResult<ExtractedResume> {
    extraction::extract_resume(path).await
}
