use super::*;

/// #1369: a job-less export (the side panel's base-résumé attach) must not invent
/// "Role"/"Company" placeholders.
#[test]
fn generate_filename_drops_the_role_and_company_placeholders_when_absent() {
    let request = ExportRequest {
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Doe".to_string()),
            job_title: None,
            company_name: Some("  ".to_string()),
            target_language: None,
        }),
        ..default_request()
    };
    assert_eq!(generate_filename(&request, "pdf"), "Jane-Doe-resume.pdf");
}
