use super::*;
use crate::ai_generations::AiGenerationRecord;
use crate::applications::ApplicationStore;

fn rec(job_url: &str) -> AiGenerationRecord {
    AiGenerationRecord {
        id: "gen-1".into(),
        created_at: 1,
        candidate_name: "Jane".into(),
        job_title: "Engineer".into(),
        company_name: "Acme".into(),
        resume_language: "en".into(),
        job_ad_language: "en".into(),
        target_language: "en".into(),
        mismatch: false,
        top_requirements: vec![],
        mode: "resume".into(),
        resume_text: "r".into(),
        cover_letter_text: String::new(),
        job_ad: String::new(),
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

/// #1358: a job-less generation must not mint an Application row.
#[test]
fn link_application_skips_the_upsert_for_a_blank_job_url() {
    let dir = tempfile::TempDir::new().unwrap();
    let apps = ApplicationStore::open(dir.path()).unwrap();
    for blank in ["", "   "] {
        let mut r = rec(blank);
        link_application(Some(&apps), &mut r);
        assert!(r.application_id.is_none());
    }
    assert!(apps.list().is_empty(), "no Application for a job-less save");
}

#[test]
fn link_application_links_a_generation_that_names_a_posting() {
    let dir = tempfile::TempDir::new().unwrap();
    let apps = ApplicationStore::open(dir.path()).unwrap();
    let mut r = rec("https://acme.example/jobs/1");
    link_application(Some(&apps), &mut r);
    assert_eq!(apps.list().len(), 1);
    assert_eq!(
        r.application_id.as_deref(),
        Some(apps.list()[0].id.as_str())
    );
}
