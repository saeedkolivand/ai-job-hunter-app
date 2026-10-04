//! Section classification — which broad kind of résumé section a heading names.
//!
//! Split out of `evidence/mod.rs` (R8's hard LOC cap); everything here moved
//! verbatim. The heading lists below are SUBSTRING (or word-bounded) tests against
//! real headings in seven languages, and the order [`classify_section`] checks them
//! in is load-bearing — read the docs on each list before touching one.

use super::contains_word;

/// Which broad kind of section a heading names. Classification only — DETECTING
/// that a line *is* a heading stays `export::parser`'s job; this just buckets the
/// heading text so evidence lands in the right list.
///
/// Public and shared with `validate::content`, which needs the same buckets: two
/// classifiers disagreeing about what "SKILLS" means would let a validator warn
/// about a section the evidence extractor filed somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Experience,
    Education,
    Projects,
    Skills,
    Summary,
    Other,
}

/// Heading fragments that name a WORK HISTORY unconditionally, matched as a
/// substring of the lowercased heading so "Berufserfahrung" and "Beruflicher
/// Werdegang" both land correctly.
/// en/de/fr/es/it/nl/pt — the languages `make_stemmer` already supports.
///
/// Every entry here says *work* ("employment", "Beruf…", "Arbeit…",
/// "…professionnelle", `werdegang` = career path, `werkervaring` = work
/// experience), which is what makes them unconditional: no other section word
/// on the same heading can outrank them. The BARE word for "experience" is not
/// here — it is ambiguous, see [`AMBIGUOUS_EXPERIENCE_HEADINGS`].
const EXPERIENCE_HEADINGS: &[&str] = &[
    "employment",
    "work experience",
    "professional experience",
    "berufserfahrung",
    "berufliche erfahrung",
    "arbeitserfahrung",
    // "Beruflicher Werdegang" is as standard a German experience heading as
    // "Berufserfahrung", and classifying it `Other` discarded every bullet
    // under it. A substring, like the compounds above: "Ausbildungswerdegang"
    // reaching Experience first is a far smaller error than losing the section.
    "werdegang",
    "expérience professionnelle",
    // Work-qualified, no second reading, no substring risk — same profile as
    // the other terms this change teaches.
    "parcours professionnel",
    "experiencia profesional",
    "esperienza professionale",
    // Italian plurals, WORK-QUALIFIED and unconditional — deliberately not the
    // bare `esperienze`, which a review proved re-creates this very commit's
    // bug one language over: "Esperienze di formazione" is an EDUCATION
    // heading, and the ambiguous set yields only to summary/skills, never to
    // education, so a bare stem there resolves Experience and files a degree
    // as a job. Qualified spellings also stay out of the section-DELETING
    // hole the ambiguous list's own doc warns about: "Esperienze
    // professionali e competenze" reaches Experience here, where the bare
    // stem would have lost it to Skills.
    "esperienze professionali",
    "esperienze lavorative",
    // Sweep finds: German "Praxiserfahrung" is not caught by the
    // word-bounded `erfahrung` (it is a compound, not a separate word), and
    // Dutch "Werkervaring" already is. Both name a work history outright.
    "praxiserfahrung",
    "werkervaring",
    "experiência profissional",
];

/// Headings that contain an EXPERIENCE stem but unambiguously name an
/// EDUCATION section, checked BEFORE the experience test so the substring
/// cannot win.
///
/// Found by sweeping every term in the TS `SECTION_LEXICON` through this
/// classifier: "Akademischer Werdegang" — a standard German heading for an
/// academic record — contains `werdegang` and so classified as Experience.
/// That is the expensive direction, not a cosmetic mislabel: prose under an
/// Experience heading reaches [`extract_evidence`]'s role arm, so degree
/// entries became work bullets under roles the candidate never held.
///
/// A closed list of exact phrases rather than a rule, deliberately. The
/// general fix — letting [`AMBIGUOUS_EXPERIENCE_HEADINGS`] yield to education
/// the way it yields to summary and skills — would overturn a documented
/// decision (see that const: Education and Projects are excluded from the
/// yield set on purpose, because "Project Experience" really is a work
/// history in a consultant's CV). Every phrase here has an explicit education
/// qualifier and so has no such second reading, which is what lets membership
/// stand in for a rule. The cost is that membership must be kept up: a review
/// found four more already in the wild after the first three were written, so
/// every entry is asserted individually below rather than sampled.
///
/// This also supersedes one line of [`EXPERIENCE_HEADINGS`]'s `werdegang`
/// note, which accepted "Ausbildungswerdegang" landing on Experience as
/// cheaper than losing the section. It now lands on Education, which is
/// better than either.
pub(super) const EDUCATION_OVERRIDES_EXPERIENCE: &[&str] = &[
    "akademischer werdegang",
    // German declines and compounds `werdegang` freely, so an exact-phrase
    // list is only as good as its membership. A review found the next four
    // already in the wild — "Wissenschaftlicher Werdegang" heads the academic
    // CV this feature is for — each of them filing a degree as a job on main
    // and on the first draft of this fix alike.
    "akademischen werdegang",
    "wissenschaftlicher werdegang",
    "wissenschaftlichen werdegang",
    "schulischer werdegang",
    "bildungswerdegang",
    "akademische laufbahn",
    "ausbildungswerdegang",
];

/// The bare German word for "experience", matched with [`contains_word`].
///
/// Word-bounded rather than a substring, and the plural is listed for the same
/// reason `formations` is: the rule is exact at BOTH ends. As a substring
/// `erfahrung` would subsume `berufserfahrung` and `arbeitserfahrung` — and
/// every other `…erfahrung` compound with it, including ones that name no work
/// history ("Nutzererfahrung" on a designer's skills heading). Bounded, it adds
/// only the spellings that were actually missing: "Erfahrung", "Erfahrungen",
/// "Berufliche Erfahrung".
///
/// Ambiguous in exactly the way [`AMBIGUOUS_EXPERIENCE_HEADINGS`] is, and read
/// under the same rule — it is a separate const only because it needs the
/// word-boundary matcher.
const EXPERIENCE_HEADINGS_WORD_BOUNDED: &[&str] = &["erfahrung", "erfahrungen"];

/// Experience stems that ALSO open a SUMMARY or a SKILLS heading, and therefore
/// lose to [`SUMMARY_HEADINGS`]/[`SKILLS_HEADINGS`] on a heading that carries
/// both.
///
/// `career` names a work history on its own ("Career", "Career History") and
/// names a *summary* just as often ("Career Summary", "Career Objective",
/// "Career Profile"). The bare word for "experience" is ambiguous the same way
/// against SKILLS: "Skills and Experience", "Technical Skills & Experience" and
/// "Kenntnisse und Erfahrungen" head a skills MATRIX in a real résumé, not a
/// list of employers. Because the experience test runs first, every one of
/// those classified as Experience — and that is not a cosmetic mislabel: prose
/// under an Experience heading reaches [`extract_evidence`]'s role arm, so a
/// summary paragraph or a skills line became a work bullet under a role the
/// candidate never had.
///
/// **The rule is scoped to these stems, not applied to the whole classifier,
/// because the two mistakes do not cost the same.** Skills filed as experience
/// invents a role — noisy, recoverable, visible. Experience filed as skills
/// DELETES the section: nothing in [`extract_evidence`] reads a Skills section
/// (no role arm, no bullet arm, and the last-resort rescue covers `Other`
/// only), so a work history under a skills-word heading would reach the
/// generation prompt as an empty evidence set. Keeping every work-qualified
/// spelling in [`EXPERIENCE_HEADINGS`] means "Berufserfahrung und Kenntnisse"
/// and "Work Experience and Skills" cannot fall into that hole, while the
/// ambiguous stems take the cheaper error.
///
/// Only SUMMARY and SKILLS are in the yield set. Education and Projects are
/// deliberately out: "Project Experience" is a work history in a consultant's
/// CV as often as it is a projects section, and neither reading has been
/// observed to cost anything yet.
const AMBIGUOUS_EXPERIENCE_HEADINGS: &[&str] = &[
    "career",
    "experience",
    "expérience",
    "experiencia",
    "esperienza",
    "experiência",
];
const EDUCATION_HEADINGS: &[&str] = &[
    "education",
    "academic",
    "ausbildung",
    "bildung",
    // Sweep find. German "Studium" is the ordinary word for an education
    // section and matched nothing. Safe as a substring: "Auslandsstudium",
    // "Selbststudium" and "Studium Generale" are all education headings.
    "studium",
    "opleiding",
];

/// Education stems that are also the tail of an ordinary word, matched with
/// [`contains_word`] instead of as a bare substring.
///
/// `formation` hides inside `information` — and `formación`/`formação`/
/// `formazione` inside `información`/`informação`/`informazione` — so the
/// "PERSONAL INFORMATION" heading that opens half the CVs in Europe classified
/// as EDUCATION, filing the candidate's phone number and email as a degree.
/// Both the singular and the plural are listed because the word-boundary rule
/// is exact at BOTH ends: without `formations`, a French "FORMATIONS" heading
/// would stop classifying, and adding it to the substring list above would
/// re-open the collision on `informations`.
///
/// Only these stems are word-bounded. Every other entry stays a substring
/// because German compounds a heading straight into a longer word
/// (`Weiterbildung` → `bildung`, `Berufsausbildung` → `ausbildung`), which a
/// both-ends boundary would break.
const EDUCATION_HEADINGS_WORD_BOUNDED: &[&str] = &[
    // Italian "Istruzione" — the very heading `resume_conventions("it")`
    // teaches the model to write, and it matched nothing. WORD-BOUNDED for
    // the same reason `formation` below is: `distruzione` contains
    // `istruzione` exactly as `information` contains `formation`.
    "istruzione",
    "istruzioni",
    "formation",
    "formations",
    "formación",
    "formaciones",
    "formação",
    "formações",
    "formazione",
    "formazioni",
];
const PROJECT_HEADINGS: &[&str] = &["project", "projekt", "projet", "proyecto", "progetti"];
const SKILLS_HEADINGS: &[&str] = &[
    "skill",
    "competenc",
    // Italian "COMPETENZE" — `competenc` (English "competencies") does not
    // cover it, and a missed skills heading silently disables every
    // skills-section check on an Italian résumé.
    "competenz",
    // Portuguese "Competências" — the `ê` blocks `competenc` exactly as the
    // `z` blocked it for Italian above. It is the pt Skills header the résumé
    // prompt itself emits, so without this a Portuguese résumé generated by
    // this app classifies its own skills section as `Other`.
    "competênc",
    "fähigkeit",
    "kenntnis",
    "kompetenz",
    "compétence",
    "habilidad",
    "vaardigheden",
    "technologies",
    "tech stack",
];
const SUMMARY_HEADINGS: &[&str] = &[
    "summary",
    "profile",
    "objective",
    "about",
    "zusammenfassung",
    "profil",
    "perfil",
    "profilo",
    "profiel",
];

/// Bucket a heading. Substring match on the lowercased heading, checked
/// most-specific-first, so "PROFESSIONAL EXPERIENCE" and "Berufserfahrung" both
/// land on [`SectionKind::Experience`] without a per-locale word list. The
/// exceptions are [`EXPERIENCE_HEADINGS_WORD_BOUNDED`] and
/// [`EDUCATION_HEADINGS_WORD_BOUNDED`], whose stems are substrings of ordinary
/// words and are therefore matched with [`contains_word`].
///
/// The one place the "experience first" order is NOT applied is
/// [`AMBIGUOUS_EXPERIENCE_HEADINGS`] (plus the word-bounded German twins) — a
/// stem that opens a summary or a skills heading as readily as an experience
/// one yields to a summary/skills word on the same heading. See that const for
/// why the yield is scoped to those stems rather than reordering the classifier.
pub fn classify_section(heading: &str) -> SectionKind {
    let lower = heading.to_lowercase();
    let has = |set: &[&str]| set.iter().any(|k| lower.contains(k));
    let has_word = |set: &[&str]| set.iter().any(|k| contains_word(&lower, k));
    let summary = has(SUMMARY_HEADINGS);
    let skills = has(SKILLS_HEADINGS);
    let ambiguous_experience =
        has(AMBIGUOUS_EXPERIENCE_HEADINGS) || has_word(EXPERIENCE_HEADINGS_WORD_BOUNDED);
    // Before the experience test, not after: these carry an experience
    // SUBSTRING, so anything downstream of that test is unreachable for them.
    if has(EDUCATION_OVERRIDES_EXPERIENCE) {
        SectionKind::Education
    } else if has(EXPERIENCE_HEADINGS) || (ambiguous_experience && !summary && !skills) {
        SectionKind::Experience
    } else if has(EDUCATION_HEADINGS) || has_word(EDUCATION_HEADINGS_WORD_BOUNDED) {
        SectionKind::Education
    } else if has(PROJECT_HEADINGS) {
        SectionKind::Projects
    } else if has(SKILLS_HEADINGS) {
        SectionKind::Skills
    } else if summary {
        SectionKind::Summary
    } else {
        SectionKind::Other
    }
}
