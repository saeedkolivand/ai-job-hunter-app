pub mod clean;
pub mod confidence;
pub mod docx;
pub mod html;
pub mod pdf;
pub mod plain;
pub mod registry;
pub mod rtf;
/// Pure `match.live` salary-range extraction (PR3) — see its module doc.
pub mod salary;
pub mod structured;
pub mod types;

use std::path::Path;

use tracing::{instrument, warn};

use crate::error::{AppError, AppResult};
use types::{ExtractedResume, ExtractionError};

const MAX_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Resume extraction entry point (text + structure). Pure logic with no Tauri
/// coupling — the IPC command wrapper lives in the shell module `commands::resume`,
/// which keeps the shell layer the sole owner of command definitions
/// (see docs/architecture-rules.md R1).
///
/// Returns `Ok(ExtractedResume)` on success. Internal details are logged
/// server-side via `tracing`; only the `Display` form of `ExtractionError`
/// reaches the frontend.
#[instrument(skip_all, fields(path))]
pub async fn extract_resume(path: String) -> AppResult<ExtractedResume> {
    tracing::Span::current().record("path", path.as_str());

    let bytes = std::fs::read(&path).map_err(|e| {
        let err = ExtractionError::IoError(e.to_string());
        warn!(%err, "failed to read file");
        AppError::from(err)
    })?;

    route(&path, &bytes).map_err(|e| {
        warn!(error = %e, "extraction failed");
        AppError::from(e)
    })
}

/// Pure (non-async) router — easier to unit-test without a Tauri runtime.
///
/// Dispatch is data-driven: the extension is resolved against the
/// [`registry`], so adding a format never touches this function.
pub fn route(path: &str, bytes: &[u8]) -> Result<ExtractedResume, ExtractionError> {
    if bytes.len() > MAX_BYTES {
        return Err(ExtractionError::FileTooLarge { size: bytes.len() });
    }

    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match registry::extractor_for(&ext) {
        Some(extractor) => extractor.extract(bytes),
        None => Err(ExtractionError::UnsupportedFormat { ext }),
    }
}

#[cfg(test)]
mod tests;
