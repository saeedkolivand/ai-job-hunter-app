/// Form selector mappings for each board's application forms.
/// These selectors need to be verified against live production UI.

#[derive(Debug, Clone)]
pub struct FormSelectors {
    pub name: Vec<String>,
    pub email: Vec<String>,
    pub phone: Vec<String>,
    pub resume_upload: Vec<String>,
    pub cover_letter: Vec<String>,
    pub submit_button: Vec<String>,
    pub captcha_detection: Vec<String>,
}

impl FormSelectors {
    pub fn linkedin() -> Self {
        Self {
            name: vec![
                "input[name='firstName']".to_string(),
                "input[name='lastName']".to_string(),
            ],
            email: vec!["input[type='email']".to_string()],
            phone: vec!["input[type='tel']".to_string()],
            resume_upload: vec![
                "input[type='file']".to_string(),
                "[data-test='resume-upload']".to_string(),
            ],
            cover_letter: vec![
                "textarea[name='coverLetter']".to_string(),
                "[data-test='cover-letter']".to_string(),
            ],
            submit_button: vec![
                "button[type='submit']".to_string(),
                "[data-test='apply-submit']".to_string(),
            ],
            captcha_detection: vec![
                "[data-test='captcha']".to_string(),
                ".captcha".to_string(),
            ],
        }
    }

    pub fn indeed() -> Self {
        Self {
            name: vec!["input[name='applicant.name']".to_string()],
            email: vec!["input[type='email']".to_string()],
            phone: vec!["input[type='tel']".to_string()],
            resume_upload: vec![
                "input[type='file']".to_string(),
                "[id='resume-upload']".to_string(),
            ],
            cover_letter: vec![
                "textarea[name='coverLetter']".to_string(),
                "[id='cover-letter']".to_string(),
            ],
            submit_button: vec![
                "button[type='submit']".to_string(),
                "[id='apply-button']".to_string(),
            ],
            captcha_detection: vec![
                ".recaptcha".to_string(),
                "[data-captcha]".to_string(),
            ],
        }
    }

    pub fn greenhouse() -> Self {
        Self {
            name: vec!["input[name='name']".to_string()],
            email: vec!["input[type='email']".to_string()],
            phone: vec!["input[type='tel']".to_string()],
            resume_upload: vec![
                "input[type='file']".to_string(),
                "[id='resume']".to_string(),
            ],
            cover_letter: vec![
                "textarea[name='cover_letter']".to_string(),
                "[id='cover_letter']".to_string(),
            ],
            submit_button: vec![
                "button[type='submit']".to_string(),
                "[id='submit-application']".to_string(),
            ],
            captcha_detection: vec![
                ".g-recaptcha".to_string(),
                "[data-sitekey]".to_string(),
            ],
        }
    }

    pub fn workday() -> Self {
        Self {
            name: vec!["input[name='candidate-name']".to_string()],
            email: vec!["input[type='email']".to_string()],
            phone: vec!["input[type='tel']".to_string()],
            resume_upload: vec![
                "input[type='file']".to_string(),
                "[data-automation-id='fileUpload']".to_string(),
            ],
            cover_letter: vec![
                "textarea[name='cover-letter']".to_string(),
                "[data-automation-id='coverLetter']".to_string(),
            ],
            submit_button: vec![
                "button[type='submit']".to_string(),
                "[data-automation-id='submit']".to_string(),
            ],
            captcha_detection: vec![
                ".captcha".to_string(),
                "[data-captcha]".to_string(),
            ],
        }
    }

    pub fn xing() -> Self {
        Self {
            name: vec!["input[name='name']".to_string()],
            email: vec!["input[type='email']".to_string()],
            phone: vec!["input[type='tel']".to_string()],
            resume_upload: vec![
                "input[type='file']".to_string(),
                "[data-qa='file-upload']".to_string(),
            ],
            cover_letter: vec![
                "textarea[name='coverLetter']".to_string(),
                "[data-qa='cover-letter']".to_string(),
            ],
            submit_button: vec![
                "button[type='submit']".to_string(),
                "[data-qa='submit-button']".to_string(),
            ],
            captcha_detection: vec![
                ".recaptcha".to_string(),
                "[data-captcha]".to_string(),
            ],
        }
    }

    pub fn glassdoor() -> Self {
        Self {
            name: vec!["input[name='name']".to_string()],
            email: vec!["input[type='email']".to_string()],
            phone: vec!["input[type='tel']".to_string()],
            resume_upload: vec![
                "input[type='file']".to_string(),
                "[data-test='resume-upload']".to_string(),
            ],
            cover_letter: vec![
                "textarea[name='coverLetter']".to_string(),
                "[data-test='cover-letter']".to_string(),
            ],
            submit_button: vec![
                "button[type='submit']".to_string(),
                "[data-test='submit-button']".to_string(),
            ],
            captcha_detection: vec![
                ".recaptcha".to_string(),
                "[data-test='captcha']".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
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
}
