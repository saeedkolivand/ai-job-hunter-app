use super::*;
use crate::ai_generations::{AiGenerationRecord, ApplicationAnswer, InterviewQuestion};
use crate::documents::DocumentRecord;

fn generation_record(resume_text: &str, cover_letter_text: &str) -> AiGenerationRecord {
    AiGenerationRecord {
        id: "gen-1".to_string(),
        created_at: 1_700_000_000_000,
        candidate_name: "Jane Candidate".to_string(),
        job_title: "  Staff Engineer  ".to_string(),
        company_name: "".to_string(),
        resume_language: "en".to_string(),
        job_ad_language: "en".to_string(),
        target_language: "en".to_string(),
        mismatch: false,
        top_requirements: vec![],
        mode: "text".to_string(),
        resume_text: resume_text.to_string(),
        cover_letter_text: cover_letter_text.to_string(),
        job_ad: String::new(),
        job_url: "https://example.com/job/1".to_string(),
        board: "linkedin".to_string(),
        application_answers: Vec::<ApplicationAnswer>::new(),
        company_brief: String::new(),
        interview_questions: Vec::<InterviewQuestion>::new(),
        email_subject: String::new(),
        email_body: String::new(),
        application_id: None,
        quality_report: String::new(),
    }
}

fn doc(id: &str, name: &str, created_at: u64, locale: Option<&str>) -> DocumentRecord {
    DocumentRecord {
        id: id.to_string(),
        title: name.to_string(),
        name: name.to_string(),
        locale: locale.map(str::to_string),
        text: "some résumé text".to_string(),
        pages: None,
        created_at,
        indexed: false,
        is_default: false,
        keywords_json: None,
    }
}

#[test]
fn project_generation_reports_presence_only_never_the_text() {
    let record = generation_record("my résumé text", "");
    let v = project_generation(&record);
    assert_eq!(v["hasResume"], true);
    assert_eq!(v["hasCoverLetter"], false);
    // Trimmed, blank company omitted entirely — never sent as `""`.
    assert_eq!(v["jobTitle"], "Staff Engineer");
    assert!(v.get("company").is_none());
    assert!(
        !v.to_string().contains("my résumé text"),
        "the projection must never carry the résumé text itself"
    );
}

#[test]
fn project_generation_reports_both_kinds_present() {
    let record = generation_record("resume text", "cover letter text");
    let v = project_generation(&record);
    assert_eq!(v["hasResume"], true);
    assert_eq!(v["hasCoverLetter"], true);
}

#[test]
fn project_document_omits_language_when_the_stored_locale_is_absent_or_blank() {
    let with_locale = doc("d1", "My Resume.pdf", 100, Some("de"));
    assert_eq!(project_document(&with_locale)["language"], "de");

    let no_locale = doc("d2", "My Resume.pdf", 100, None);
    assert!(project_document(&no_locale).get("language").is_none());

    let blank_locale = doc("d3", "My Resume.pdf", 100, Some("   "));
    assert!(project_document(&blank_locale).get("language").is_none());
}
