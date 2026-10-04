//! Fixtures shared by the lexical-search tests.

use super::*;

pub(super) fn doc<'a>(
    id: &'a str,
    title: &'a str,
    company: &'a str,
    description: &'a str,
) -> LexicalDoc<'a> {
    LexicalDoc {
        id,
        title,
        company,
        location: "",
        description,
    }
}

/// Two postings, only one of which mentions golang: for the checks that FTS5
/// operator characters in a query stay literal text.
pub(super) fn golang_docs<'a>() -> Vec<LexicalDoc<'a>> {
    vec![
        doc(
            "p1",
            "Engineer",
            "Acme",
            "Experience with golang and Kubernetes.",
        ),
        doc("p2", "Designer", "Beta", "No backend experience."),
    ]
}
