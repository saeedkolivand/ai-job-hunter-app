//! Fixtures shared by the `InteractionStore` tests.

use super::*;

/// A fresh store over a temp data dir. Hold the guard for as long as the store is used.
pub(super) fn open_store() -> (TempDir, PathBuf, InteractionStore) {
    let dir = TempDir::new().unwrap();
    let data_dir = dir.path().to_path_buf();
    let store = InteractionStore::new(&data_dir);
    (dir, data_dir, store)
}

pub(super) fn interaction(job_id: &str, interaction_type: &str) -> InteractionRecord {
    InteractionRecord {
        job_id: job_id.to_string(),
        interaction_type: interaction_type.to_string(),
        timestamp: 0,
        title: "Test".to_string(),
        company: "Test".to_string(),
        url: "https://example.com".to_string(),
        source: "test".to_string(),
        location: "Remote".to_string(),
    }
}
