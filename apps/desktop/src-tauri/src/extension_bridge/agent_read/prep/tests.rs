use super::*;
use crate::ai_generations::{AiGenerationRecord, ApplicationAnswer, InterviewQuestion};

fn base_record() -> AiGenerationRecord {
    AiGenerationRecord {
        id: "gen-1".to_string(),
        created_at: 1_700_000_000_000,
        candidate_name: "Jane Candidate".to_string(),
        job_title: "Staff Engineer".to_string(),
        company_name: "Acme".to_string(),
        resume_language: "en".to_string(),
        job_ad_language: "en".to_string(),
        target_language: "en".to_string(),
        mismatch: false,
        top_requirements: vec![],
        mode: "text".to_string(),
        resume_text: "resume".to_string(),
        cover_letter_text: String::new(),
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

fn question(id: &str, question: &str, why: &str, audience: &str) -> InterviewQuestion {
    InterviewQuestion {
        id: id.to_string(),
        question: question.to_string(),
        why: why.to_string(),
        audience: audience.to_string(),
    }
}

fn answer(id: &str, question: &str, answer: &str) -> ApplicationAnswer {
    ApplicationAnswer {
        id: id.to_string(),
        question: question.to_string(),
        answer: answer.to_string(),
    }
}

#[test]
fn nothing_generated_reports_absent_brief_empty_questions_no_salary() {
    let record = base_record();
    let v = project_generation(&record);
    assert_eq!(v["hasCompanyBrief"], false);
    assert!(v.get("companyBrief").is_none());
    assert_eq!(v["interviewQuestions"].as_array().unwrap().len(), 0);
    assert!(v.get("salaryAnswer").is_none());
    assert_eq!(v["truncated"], false);
    assert_eq!(v["updatedAt"], 1_700_000_000_000_u64);
}

#[test]
fn everything_present_returns_the_actual_text_unlike_the_documents_resource() {
    let mut record = base_record();
    record.company_brief = "Acme builds payment infrastructure.".to_string();
    record.interview_questions = vec![question(
        "q1",
        "What excites you about this role?",
        "Shows genuine interest",
        "recruiter",
    )];
    record.application_answers = vec![answer("salary", "Salary expectations?", "$150k-$170k")];

    let v = project_generation(&record);
    assert_eq!(v["hasCompanyBrief"], true);
    assert_eq!(v["companyBrief"], "Acme builds payment infrastructure.");
    let questions = v["interviewQuestions"].as_array().unwrap();
    assert_eq!(questions.len(), 1);
    assert_eq!(
        questions[0]["question"],
        "What excites you about this role?"
    );
    assert_eq!(questions[0]["why"], "Shows genuine interest");
    assert_eq!(questions[0]["audience"], "recruiter");
    assert_eq!(v["salaryAnswer"], "$150k-$170k");
    assert_eq!(v["truncated"], false);
}

#[test]
fn salary_answer_is_matched_by_the_fixed_salary_question_id_only() {
    let mut record = base_record();
    record.application_answers = vec![
        answer("q-visa", "Do you need sponsorship?", "No"),
        answer("salary", "What are your expectations?", "$120k"),
    ];
    let v = project_generation(&record);
    assert_eq!(v["salaryAnswer"], "$120k");
}

#[test]
fn a_blank_salary_answer_text_is_treated_as_absent() {
    let mut record = base_record();
    record.application_answers = vec![answer("salary", "Salary?", "   ")];
    let v = project_generation(&record);
    assert!(v.get("salaryAnswer").is_none());
}

#[test]
fn interview_questions_over_the_cap_are_truncated_with_the_flag_set() {
    let mut record = base_record();
    record.interview_questions = (0..(MAX_INTERVIEW_QUESTIONS + 3))
        .map(|i| question(&format!("q{i}"), &format!("Question {i}"), "why", "team"))
        .collect();
    let v = project_generation(&record);
    assert_eq!(
        v["interviewQuestions"].as_array().unwrap().len(),
        MAX_INTERVIEW_QUESTIONS
    );
    assert_eq!(v["truncated"], true);
}

#[test]
fn an_overlong_company_brief_is_clamped_with_the_flag_set() {
    let mut record = base_record();
    record.company_brief = "x".repeat(MAX_BRIEF_CHARS + 500);
    let v = project_generation(&record);
    assert_eq!(
        v["companyBrief"].as_str().unwrap().chars().count(),
        MAX_BRIEF_CHARS
    );
    assert_eq!(v["truncated"], true);
}

#[test]
fn an_interview_question_missing_why_or_audience_omits_them_rather_than_sending_blank() {
    let mut record = base_record();
    record.interview_questions = vec![question("q1", "Tell me about yourself", "", "")];
    let v = project_generation(&record);
    let q0 = &v["interviewQuestions"][0];
    assert!(q0.get("why").is_none());
    assert!(q0.get("audience").is_none());
}
