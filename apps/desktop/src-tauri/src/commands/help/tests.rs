use super::*;

// ── Fixtures ─────────────────────────────────────────────────────────────────

pub(super) fn entry(id: &str, title: &str, body: &str) -> HelpSearchRequestEntry {
    HelpSearchRequestEntry {
        id: id.to_string(),
        title: title.to_string(),
        body: body.to_string(),
    }
}

/// A small, realistic corpus: three entries whose ANSWERS carry wording the
/// questions do not, so a query written against an answer can only reach its
/// entry through the `description` column.
pub(super) fn corpus() -> Vec<HelpSearchRequestEntry> {
    vec![
        entry(
            "documentsQuestions.importFormats",
            "Which file formats can I import?",
            "Drop a file onto Resume Management: the hint reads PDF, DOC, DOCX, and TXT files \
             are accepted too. A scanned page has no text layer to extract.",
        ),
        entry(
            "aiGenerateQuestions.exportDoc",
            "How do I export a finished document?",
            "Press Export above a finished document and choose PDF, DOCX or TXT.",
        ),
        entry(
            "privacyQuestions.whatLeaves",
            "What data leaves my computer?",
            "Your documents and applications stay in local files on your machine.",
        ),
    ]
}

mod lexical;
mod reply;
mod source_scans;
mod validation;
