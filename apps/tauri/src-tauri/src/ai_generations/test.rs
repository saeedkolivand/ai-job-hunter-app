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
        application_answers: vec![],
        company_brief: String::new(),
    }
}

fn answer(id: &str) -> ApplicationAnswer {
    ApplicationAnswer {
        id: id.into(),
        question: format!("Q-{id}"),
        answer: format!("A-{id}"),
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
    assert!(list[0].application_answers.is_empty());
    assert_eq!(list[0].company_brief, "");
}

#[test]
fn insert_round_trips_answers_and_brief() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let mut rec = record("g1", "https://acme.com/job/1");
    rec.application_answers = vec![answer("why-company"), answer("strengths")];
    rec.company_brief = "Acme builds payment rails.".into();
    store.insert(&rec).unwrap();

    let list = store.list();
    assert_eq!(list[0].application_answers, rec.application_answers);
    assert_eq!(list[0].company_brief, "Acme builds payment rails.");
}

#[test]
fn merge_layers_answers_onto_an_existing_cover_without_clobbering() {
    let mut existing = record("g1", "https://acme.com/job/1");
    existing.cover_letter_text = "COVER".into();
    existing.application_answers = vec![];

    // An answers-only save: empty résumé/cover, but carries answers + brief.
    let mut incoming = record("g2", "https://acme.com/job/1");
    incoming.resume_text = String::new();
    incoming.cover_letter_text = String::new();
    incoming.application_answers = vec![answer("why-company")];
    incoming.company_brief = "brief".into();

    let merged = merge_application(existing, incoming);

    assert_eq!(merged.id, "g1", "keeps the existing row id");
    assert_eq!(merged.cover_letter_text, "COVER", "cover is not wiped");
    assert_eq!(merged.application_answers, vec![answer("why-company")]);
    assert_eq!(merged.company_brief, "brief");
}

#[test]
fn save_application_upserts_by_job_url_into_one_aggregate() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    let url = "https://acme.com/job/1";

    // First the tailor flow saves a résumé/cover for the job.
    store.save_application(record("g1", url)).unwrap();

    // Then the questions assistant saves answers (no résumé/cover) for the same job.
    let mut answers_save = record("g2", url);
    answers_save.resume_text = String::new();
    answers_save.cover_letter_text = String::new();
    answers_save.application_answers = vec![answer("why-company")];
    store.save_application(answers_save).unwrap();

    let list = store.list();
    assert_eq!(list.len(), 1, "one aggregate row per job");
    assert_eq!(list[0].cover_letter_text, "C", "cover preserved");
    assert_eq!(list[0].application_answers, vec![answer("why-company")]);
}

#[test]
fn save_application_inserts_separate_rows_when_unlinked() {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    store.save_application(record("g1", "")).unwrap();
    store.save_application(record("g2", "")).unwrap();
    assert_eq!(
        store.list().len(),
        2,
        "manual (unlinked) saves stay separate"
    );
}
