//! Fixtures shared by the `email_watch` submodules' own tests.

use crate::applications::{Application, ApplicationStatus};
use crate::email_watch::parser::EmailHeader;

/// An otherwise-empty [`Application`] with the fields the matcher/poller key on.
pub(in crate::email_watch) fn application(
    id: &str,
    company: &str,
    title: &str,
    status: ApplicationStatus,
) -> Application {
    Application {
        id: id.to_string(),
        status,
        applied_at: None,
        created_at: 0,
        updated_at: 0,
        job_url: String::new(),
        board: String::new(),
        company: company.to_string(),
        title: title.to_string(),
        candidate: String::new(),
        answers: Vec::new(),
        brief: String::new(),
        job_description: String::new(),
        notes: String::new(),
        next_action_at: None,
        next_action_notified_at: None,
        comp: String::new(),
        contact_name: String::new(),
        contact_email: String::new(),
        job_summary: String::new(),
        recipient_name: String::new(),
        recipient_email: String::new(),
        salary_min: None,
        salary_max: None,
        salary_currency: None,
    }
}

/// A parsed header carrying only the fields the tests vary.
pub(in crate::email_watch) fn email_header(
    subject: &str,
    from_domain: Option<&str>,
    dmarc_pass: bool,
) -> EmailHeader {
    EmailHeader {
        subject: subject.to_string(),
        from_name: None,
        from_domain: from_domain.map(str::to_string),
        message_id: None,
        dmarc_pass,
    }
}
