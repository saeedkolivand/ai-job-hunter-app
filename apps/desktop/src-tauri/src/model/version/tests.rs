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
