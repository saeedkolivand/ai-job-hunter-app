use super::*;
use crate::applying::selectors::FormSelectors;

#[test]
fn test_form_filler_new() {
    let selectors = FormSelectors::linkedin();
    let filler = FormFiller::new(selectors);
    // Just verify it constructs without panicking
    let _ = filler;
}

#[test]
fn test_form_filler_new_with_custom_selectors() {
    let selectors = FormSelectors {
        name: vec!["#name".to_string()],
        email: vec!["#email".to_string()],
        phone: vec!["#phone".to_string()],
        resume_upload: vec!["#resume".to_string()],
        cover_letter: vec!["#cover-letter".to_string()],
        submit_button: vec!["#submit".to_string()],
        captcha_detection: vec![".captcha".to_string()],
    };
    let filler = FormFiller::new(selectors);
    assert!(!filler.selectors.name.is_empty());
}

#[test]
fn test_form_filler_selectors_field_count() {
    let selectors = FormSelectors::linkedin();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
}
