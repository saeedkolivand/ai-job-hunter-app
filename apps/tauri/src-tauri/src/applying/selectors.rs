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
}
