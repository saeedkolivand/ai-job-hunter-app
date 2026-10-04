//! Internal schema versioning for [`DocumentModel`]. The external IPC contract
//! (`ExportRequest`) stays stable; internal model evolution is absorbed by
//! `schema_version` + [`migrate`].

use super::document::DocumentModel;

/// Current canonical-model schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Migrate a model to the current schema version. Identity for v1; future
/// schema bumps add forward-migration steps here before stamping the new version.
pub fn migrate(mut model: DocumentModel) -> DocumentModel {
    if model.schema_version != SCHEMA_VERSION {
        // No older schemas exist yet; future arms transform `model` in place.
        model.schema_version = SCHEMA_VERSION;
    }
    model
}

#[cfg(test)]
mod tests;
