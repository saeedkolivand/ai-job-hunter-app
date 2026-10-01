//! Shared fixtures (dummy_style, EN_LETTER) for the letterhead-guard test topics.

use super::super::super::letter::model::LetterStyle;

pub(super) fn dummy_style() -> LetterStyle {
    LetterStyle {
        c_accent: "#2563EB".to_string(),
        c_body: "#222222".to_string(),
        c_name: "#111111".to_string(),
        c_date: "#555555".to_string(),
        c_rule: "#aaaaaa".to_string(),
        font_name: "Carlito".to_string(),
        font_body: "Carlito".to_string(),
        name_pt: 20.0,
        body_pt: 10.5,
    }
}

pub(super) const EN_LETTER: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my interest in the Software Engineer position at Acme Corp. \
I have five years of experience building distributed systems.

During my time at Beta Inc, I led the migration of our payments service, reducing \
latency by 40 percent and cutting costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with your needs.

Sincerely,

Jane Smith
Software Engineer
";
