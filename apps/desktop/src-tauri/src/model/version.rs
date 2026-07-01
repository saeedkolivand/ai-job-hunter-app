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
mod tests {
    use super::*;
    use crate::export::types::DocumentType;

    #[test]
    fn migrate_is_identity_for_current_version() {
        let model = DocumentModel::new(DocumentType::Resume);
        let migrated = migrate(model.clone());
        assert_eq!(migrated, model);
        assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_stamps_current_version_on_mismatch() {
        let mut model = DocumentModel::new(DocumentType::CoverLetter);
        model.schema_version = 0;
        assert_eq!(migrate(model).schema_version, SCHEMA_VERSION);
    }
}
