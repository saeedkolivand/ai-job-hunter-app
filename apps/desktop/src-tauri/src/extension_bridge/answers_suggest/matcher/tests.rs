//! Split by topic (R8 relief — redistributed from the crate-level `import_tests.rs`, this unit's
//! own tests alone exceed the LOC cap): `ranking` covers `match_questions`'s scoring/tie-break/
//! cap behaviour; `salary_en_keywords`/`salary_en_phrasing`/`salary_de` cover the salary-keyword
//! flag across English keyword/phrasing variants and German (DACH) shapes. The shared
//! `candidate` fixture builder below.

use super::*;

mod ranking;
mod salary_de;
mod salary_en_keywords;
mod salary_en_phrasing;

pub(super) fn candidate<'a>(
    question: &'a str,
    answer: &'a str,
    company: &'a str,
    title: &'a str,
    updated_at: u64,
) -> AnswerCandidate<'a> {
    AnswerCandidate::new(question, answer, company, title, updated_at)
}
