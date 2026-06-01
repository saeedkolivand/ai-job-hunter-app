use super::*;
use tempfile::TempDir;

fn record(id: &str, job_url: &str) -> AiGenerationRecord {
    AiGenerationRecord {
        id: id.into(),
        created_at: now_ms(),
        candidate_name: "Jane".into(),
        job_title: "Engineer".into(),
        company_name: "Acme".into(),
        resume_language: "en".into(),
        job_ad_language: "en".into(),
        target_language: "en".into(),
        mismatch: false,
        top_requirements: vec!["rust".into()],
        mode: "ats".into(),
        resume_text: "R".into(),
        cover_letter_text: "C".into(),
        job_ad: "JD".into(),
        job_url: job_url.into(),
        board: "linkedin".into(),
    }
}

#[test]
fn insert_round_trips_the_job_link() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store
        .insert(&record("g1", "https://acme.com/job/1"))
        .unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].job_url, "https://acme.com/job/1");
    assert_eq!(list[0].board, "linkedin");
}

#[test]
fn applied_job_urls_returns_only_non_empty_links() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store
        .insert(&record("g1", "https://acme.com/job/1"))
        .unwrap();
    store
        .insert(&record("g2", "https://acme.com/job/2"))
        .unwrap();
    store.insert(&record("g3", "")).unwrap(); // manual generation, no job link

    let urls = store.applied_job_urls();
    assert_eq!(urls.len(), 2);
    assert!(urls.contains("https://acme.com/job/1"));
    assert!(urls.contains("https://acme.com/job/2"));
    assert!(!urls.contains(""));
}

#[test]
fn migration_defaults_link_fields_for_legacy_records() {
    // A record exported before the link columns existed (no jobUrl/board) must
    // import via serde defaults, not fail.
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let legacy = serde_json::json!([{
        "id": "old-1",
        "createdAt": 1,
        "candidateName": "Jane",
        "jobTitle": "Engineer",
        "companyName": "Acme",
        "resumeLanguage": "en",
        "jobAdLanguage": "en",
        "targetLanguage": "en",
        "mismatch": false,
        "topRequirements": [],
        "mode": "ats",
        "resumeText": "",
        "coverLetterText": "",
        "jobAd": ""
    }]);
    let n = crate::data_store::DataStore::import(&store, &legacy).unwrap();
    assert_eq!(n, 1);
    let list = store.list();
    assert_eq!(list[0].job_url, "");
    assert_eq!(list[0].board, "");
}
