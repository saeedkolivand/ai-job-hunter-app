use super::*;

#[test]
fn test_form_selectors_linkedin() {
    let selectors = FormSelectors::linkedin();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
    assert!(!selectors.captcha_detection.is_empty());
}

#[test]
fn test_form_selectors_indeed() {
    let selectors = FormSelectors::indeed();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
}

#[test]
fn test_form_selectors_greenhouse() {
    let selectors = FormSelectors::greenhouse();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
}

#[test]
fn test_form_selectors_workday() {
    let selectors = FormSelectors::workday();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
}

#[test]
fn test_form_selectors_xing() {
    let selectors = FormSelectors::xing();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
}

#[test]
fn test_form_selectors_glassdoor() {
    let selectors = FormSelectors::glassdoor();
    assert!(!selectors.name.is_empty());
    assert!(!selectors.email.is_empty());
    assert!(!selectors.submit_button.is_empty());
}

#[test]
fn test_form_selectors_clone() {
    let selectors = FormSelectors::linkedin();
    let cloned = selectors.clone();
    assert_eq!(selectors.name.len(), cloned.name.len());
}

#[test]
fn test_form_selectors_custom() {
    let selectors = FormSelectors {
        name: vec!["#custom-name".to_string()],
        email: vec!["#custom-email".to_string()],
        phone: vec![],
        resume_upload: vec![],
        cover_letter: vec![],
        submit_button: vec!["#custom-submit".to_string()],
        captcha_detection: vec![],
    };
    assert_eq!(selectors.name, vec!["#custom-name".to_string()]);
    assert_eq!(selectors.email, vec!["#custom-email".to_string()]);
}
