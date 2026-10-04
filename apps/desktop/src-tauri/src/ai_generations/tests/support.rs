//! Fixtures shared by the `ai_generations` store tests.

use super::*;

/// A fresh store in a temp dir. Hold the guard for as long as the store is used.
pub(super) fn open_store() -> (TempDir, AiGenerationStore) {
    let dir = TempDir::new().unwrap();
    let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
    (dir, store)
}

/// A raw connection to `dir`'s `ai_generations.db`, migrated to JUST BEFORE the migration
/// named `name` (the PREVIOUS schema version). Looked up BY NAME rather than "all but the
/// last" so it stays correct regardless of what gets appended after it. Returns the
/// migration's index too. Drop the connection before opening the store for real.
pub(super) fn conn_before(dir: &TempDir, name: &str) -> (Connection, usize) {
    let idx = migrations::MIGRATIONS
        .iter()
        .position(|m| m.name == name)
        .unwrap_or_else(|| panic!("{name} must still be registered"));
    let mut conn = crate::db::open(&dir.path().join("ai_generations.db")).unwrap();
    crate::db::run_migrations(&mut conn, &migrations::MIGRATIONS[..idx]).unwrap();
    (conn, idx)
}

/// Insert `record(id, job_url)` through the plain `insert` path (no per-job merge).
pub(super) fn insert(store: &AiGenerationStore, id: &str, job_url: &str) {
    store.insert(&record(id, job_url)).unwrap();
}

pub(super) fn record(id: &str, job_url: &str) -> AiGenerationRecord {
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
        interview_questions: vec![],
        email_subject: String::new(),
        email_body: String::new(),
        application_id: None,
        quality_report: String::new(),
    }
}

/// [`record`] with no résumé/cover text — what an answers-only, interview-only or
/// email-only save looks like.
pub(super) fn content_less(id: &str, job_url: &str) -> AiGenerationRecord {
    AiGenerationRecord {
        resume_text: String::new(),
        cover_letter_text: String::new(),
        ..record(id, job_url)
    }
}

pub(super) fn answer(id: &str) -> ApplicationAnswer {
    ApplicationAnswer {
        id: id.into(),
        question: format!("Q-{id}"),
        answer: format!("A-{id}"),
    }
}

pub(super) fn interview_question(id: &str) -> InterviewQuestion {
    InterviewQuestion {
        id: id.into(),
        question: format!("Q-{id}"),
        why: format!("why-{id}"),
        audience: "recruiter".into(),
    }
}
