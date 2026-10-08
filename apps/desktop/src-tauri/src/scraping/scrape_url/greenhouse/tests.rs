use super::*;

/// Trimmed from a real `boards-api.greenhouse.io/v1/boards/gitlab/jobs/<id>` response.
const REAL: &str = r#"{"absolute_url":"https://job-boards.greenhouse.io/gitlab/jobs/8638232002","location":{"name":"Remote, United States"},"id":8638232002,"updated_at":"2026-09-29T10:38:50-04:00","title":"AI Transformation Owner, CRO","company_name":"GitLab","content":"&lt;p&gt;GitLab is the intelligent orchestration platform.&lt;/p&gt;"}"#;

fn build(json: &str) -> JobPosting {
    let v = serde_json::from_str(json).unwrap();
    posting_from_api(
        "https://boards.greenhouse.io/gitlab/jobs/8638232002",
        "gitlab".into(),
        "8638232002".into(),
        &v,
    )
}

#[test]
fn company_is_the_display_name_from_the_job_response() {
    assert_eq!(build(REAL).company, "GitLab");
}

#[test]
fn company_falls_back_to_a_title_cased_slug() {
    let no_name = REAL.replace(r#""company_name":"GitLab","#, "");
    assert_eq!(build(&no_name).company, "Gitlab");
    assert_eq!(title_case_slug("acme-corp_inc"), "Acme Corp Inc");
}

#[test]
fn blank_company_name_falls_back_to_the_slug() {
    for blank in [r#""company_name":"","#, r#""company_name":"  ","#] {
        let json = REAL.replace(r#""company_name":"GitLab","#, blank);
        assert_eq!(build(&json).company, "Gitlab", "{blank}");
    }
}
