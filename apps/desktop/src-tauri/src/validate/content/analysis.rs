//! The analysis context every validator shares: the parsed sections of both
//! documents and the one stemming / language decision they are compared under.

use std::collections::{HashMap, HashSet};

use rust_stemmers::Stemmer;

use crate::documents::evidence::{classify_section, trailing_date_column, SectionKind};
use crate::documents::keywords::{
    display_forms_for_lang, keyword_coverage, keywords_for_lang, keywords_normalized_for_lang,
    languages_align, make_stemmer,
};
use crate::export::parser::parse_resume;
use crate::export::types::{LineKind, ParsedLine};

use super::language::MIN_CHARS_FOR_LANGUAGE_CHECK;
use super::{document_language_mismatch, word_count, ContentInput, DocKind};

// ── Analysis context ────────────────────────────────────────────────────────

/// One parsed section of a document: everything from a heading up to the next.
pub(crate) struct Section {
    /// `None` for the leading band before the first heading (name + contact).
    pub heading: Option<String>,
    pub kind: SectionKind,
    pub lines: Vec<ParsedLine>,
}

impl Section {
    pub fn bullets(&self) -> impl Iterator<Item = &ParsedLine> {
        self.lines
            .iter()
            .filter(|l| matches!(l.kind, LineKind::Bullet))
    }
}

/// Tokenize `text` under one stemming decision: stemmed when `aligned`, else
/// normalized only. The single answer every context below routes through.
fn tokenize(text: &str, lang: &str, aligned: bool, stemmer: &Stemmer) -> HashSet<String> {
    if aligned {
        keywords_for_lang(text, lang, stemmer)
    } else {
        keywords_normalized_for_lang(text, lang)
    }
}

/// The stemming decision for a comparison the POSTING is not a party to.
///
/// [`Analysis::tokens`] answers the résumé↔posting question, and its decision is
/// `languages_align(job_ad, target_language)`. Three checks compare texts this
/// report OWNS against each other — a document's skills section against its own
/// experience, a generated title against the source title at the same employer,
/// two bullets of one document — and for those the ad is a third party whose
/// language silently decided whether two halves of one comparison could match.
/// An English ad for a German-language role (the ordinary DACH case) switched
/// stemming off and made every German declension pair look like a mismatch.
///
/// So the decision is taken on the pair actually being compared and the stemmer
/// is read from the SAME text the decision was read from — the
/// `documents::evidence::JobVocabulary` pattern — so the two can never disagree
/// about which language is being stemmed. Both sides of every comparison are
/// stemmed or neither is.
///
/// The text is the GENERATED document, against [`Analysis::lang`]: it is the one
/// the target language is a statement about, this module already trusts that to
/// pick a function-word list, and [`Analysis::language_mismatch`] is what
/// withdraws the trust. Detection alone was rejected for the R5-F2 reason —
/// `whatlang` misreads terse tech résumés, and a misread would silently pick the
/// wrong Snowball algorithm; under this pairing a misread simply fails
/// `languages_align` and falls back to unstemmed.
pub(crate) struct DocumentTokens {
    stemmer: Stemmer,
    aligned: bool,
    /// The resolved target language, pinned at construction from a
    /// document-level decision — NOT re-detected per call. [`Self::tokens`]
    /// and [`Self::display`] are called on short per-line/per-title/per-bullet
    /// fragments throughout `consistency`/`duplicates`, and `whatlang` reading
    /// an isolated short fragment is unreliable (a stray line can read as an
    /// entirely different language at low confidence). Pinning the stopword
    /// language here keeps every fragment's tokenization — and its display
    /// map — under the SAME decision this document resolved to, the same
    /// reason `stemmer`/`aligned` are frozen once rather than redetected.
    lang: String,
}

impl DocumentTokens {
    fn of(text: &str, lang: &str) -> Self {
        Self {
            aligned: languages_align(text, lang),
            stemmer: make_stemmer(text),
            lang: lang.to_string(),
        }
    }

    /// Tokenize `text` under this decision. Stemming can only MERGE tokens, so
    /// every consumer of this that reports a difference (a skill with no
    /// backing, a drifted title) can only ever go quieter, never louder.
    pub fn tokens(&self, text: &str) -> HashSet<String> {
        tokenize(text, &self.lang, self.aligned, &self.stemmer)
    }

    /// Stem → readable form for the tokens of `text`, under the SAME decision
    /// [`Self::tokens`] used. A caller that names a token in a message owes its
    /// readable form to the decision it tokenized under: two maps keyed on
    /// different stemmers is how a display form comes back as a stem.
    pub fn display(&self, text: &str) -> HashMap<String, String> {
        if self.aligned {
            display_forms_for_lang(text, &self.lang, &self.stemmer)
        } else {
            HashMap::new()
        }
    }
}

/// Everything the validators share, resolved once.
///
/// The single `aligned` decision matters: `alignment` compares the generated
/// document's coverage against the SOURCE's coverage of the same posting, and
/// two coverages computed under different stemming rules are not comparable.
/// It is derived from `target_language` (what both documents are supposed to be
/// written in), not from each document's own detected language.
pub(crate) struct Analysis<'a> {
    pub input: &'a ContentInput<'a>,
    /// `target_language` narrowed to a 2-char lowercase ISO-639-1 code.
    pub lang: String,
    pub generated_sections: Vec<Section>,
    pub source_sections: Vec<Section>,
    pub aligned: bool,
    pub stemmer: Stemmer,
    /// The stemming decision for the checks the POSTING is not a party to. See
    /// [`DocumentTokens`].
    pub document: DocumentTokens,
    pub job_keywords: HashSet<String>,
    pub generated_keywords: HashSet<String>,
    pub source_keywords: HashSet<String>,
    /// The generated text is not in the target language. Every posting
    /// comparison is suppressed while this holds — coverage across two
    /// languages is noise, and a cascade of derived warnings would bury the one
    /// finding that matters.
    pub language_mismatch: bool,
}

impl<'a> Analysis<'a> {
    pub fn new(input: &'a ContentInput<'a>) -> Self {
        let lang = normalize_language(input.target_language);
        // ONE alignment decision for every résumé↔posting comparison in this
        // report, taken by the same `languages_align` kernel `score_one` and
        // `rank_bullets` route through. What that guarantees is SYMMETRIC
        // NORMALIZATION — both sides of every comparison here are stemmed, or
        // neither is, on the same rule the match score uses. It does not make
        // this report's numbers equal to the match score: they count different
        // corpora (a generated document vs. a stored résumé) and round
        // differently.
        let aligned = languages_align(input.job_ad, &lang);
        let stemmer = make_stemmer(input.job_ad);
        let tokens = |text: &str| tokenize(text, &lang, aligned, &stemmer);
        Self {
            document: DocumentTokens::of(input.generated, &lang),
            generated_sections: split_sections(input.generated, input.doc_kind),
            // Always a résumé, whatever kind is being VALIDATED: a cover letter
            // is still measured against the candidate's own résumé, and that
            // document still needs the Title-Case heading repair.
            source_sections: split_sections(input.source_resume, DocKind::Resume),
            lang: lang.clone(),
            job_keywords: tokens(input.job_ad),
            generated_keywords: tokens(input.generated),
            source_keywords: tokens(input.source_resume),
            language_mismatch: document_language_mismatch(
                input.generated,
                input.source_resume,
                input.job_ad,
                input.target_language,
            ),
            aligned,
            stemmer,
            input,
        }
    }

    /// Tokenize `text` the same way both sides of this report were tokenized.
    ///
    /// Stems are an implementation detail of a comparison and must never reach a
    /// message: a token taken straight from here reads as `kubernet`,
    /// `develop`, `entwickl`, and telling a user their résumé never demonstrates
    /// "kubernet" is a finding they cannot act on. This context used to carry a
    /// stem → readable map for that, built over all three documents under THIS
    /// (posting-keyed) alignment decision — but the one check that interpolated
    /// a token into a message is `consistency::skill_not_demonstrated`, which
    /// compares a document against ITSELF and therefore needs its own
    /// document-keyed stemmer AND its own map to match. Two maps keyed on
    /// different stemmers is how a display form comes back as a stem, so this
    /// one is gone rather than kept for a caller that no longer exists. Any
    /// future check that names a token owes its readable form to the same
    /// decision it tokenized under.
    pub fn tokens(&self, text: &str) -> HashSet<String> {
        tokenize(text, &self.lang, self.aligned, &self.stemmer)
    }

    /// Coverage of the posting by `tokens`, 0–100. `None` when the posting has
    /// no extractable keywords (a sparse or garbled ad) — the caller must go
    /// quiet rather than report 0%.
    pub fn coverage(&self, tokens: &HashSet<String>) -> Option<f64> {
        keyword_coverage(&self.job_keywords, tokens).map(|(c, _)| c)
    }

    /// Whether every posting comparison should be skipped: nothing extractable
    /// on the posting side, or the output is not in the target language.
    ///
    /// Deliberately does NOT include [`Self::posting_language_diverges`] —
    /// see that method for the counter-example.
    pub fn posting_comparable(&self) -> bool {
        !self.job_keywords.is_empty() && !self.language_mismatch
    }

    /// The posting is RELIABLY in a language other than the target's.
    ///
    /// A check that INTERSECTS the posting's vocabulary with a document's — as
    /// opposed to comparing two documents' coverage of the posting against each
    /// other — is meaningless when the two are written in different languages:
    /// it counts how many foreign words happen to appear in native prose, gets
    /// ~0, and reports that as a finding about the document. That is the
    /// ordinary DACH case, where an English-language ad advertises a
    /// German-speaking role: nothing is wrong with either text, and
    /// `voice.generic_letter` told the candidate their letter "could have been
    /// sent to anyone" while it named the posting's own subject matter in every
    /// sentence.
    ///
    /// **Not folded into [`Self::posting_comparable`]**, though it looks like a
    /// third premise of the same rule. `aligned` compares the ad to the TARGET
    /// LANGUAGE, which is a user setting, not to the documents — and when the
    /// target disagrees with everything else
    /// (`language_critical_is_withheld_when_the_source_reads_the_same_way`: a
    /// German ad, a German source, a German output, `target_language: "en"`)
    /// the ad and the documents still match each other, so coverage and the
    /// source-vs-generated alignment comparison are real measurements that must
    /// survive. Only the ad↔document INTERSECTION is invalid there, and only
    /// `generic_letter` computes one.
    ///
    /// The reliability half is the same R5-F2 concern the language Critical
    /// guards against: `languages_align` answers `false` for a MISDETECTED
    /// language just as readily as for a real one, and a terse ad ("Terraform
    /// AWS PostgreSQL Kubernetes platform engineer") is a keyword soup the
    /// detector reads as anything at all. This check stays on the SAME
    /// [`MIN_CHARS_FOR_LANGUAGE_CHECK`] floor `language.rs`'s guards used to
    /// share, rather than `detected_language`'s confidence gate: `aligned`
    /// (above) is computed from `languages_align`, which exposes no
    /// confidence signal to gate on, so length is the only reliability proxy
    /// available here. Suppressing on a short-ad guess would switch the check
    /// off for ordinary short postings.
    pub(super) fn posting_language_diverges(&self) -> bool {
        !self.aligned && significant_chars(self.input.job_ad) >= MIN_CHARS_FOR_LANGUAGE_CHECK
    }

    pub fn section_of_kind(&self, kind: SectionKind) -> Option<&Section> {
        self.generated_sections.iter().find(|s| s.kind == kind)
    }

    /// EVERY generated section of `kind`, not just the first.
    ///
    /// [`Self::section_of_kind`] answers "the" section — a reasonable
    /// convenience for a check that only ever needs to know ONE occurrence
    /// exists (an empty-section warning, a bullet-count check). It is NOT
    /// reasonable for a check that has to see everything the document claims:
    /// an invented project link that lands in a SECOND Projects section is
    /// invisible to `.find()`, and `factual::project_link_issues` is
    /// Critical-severity, so that blind spot is a real fabrication going
    /// unreported rather than a cosmetic miss. Use this there.
    ///
    /// The two remaining `section_of_kind` consumers
    /// (`consistency::skill_not_demonstrated_issues`,
    /// `consistency::project_structure_issues`) stay on the single-section
    /// form: both are Warning-severity, and the duplicate-section case this
    /// closes is a repair/humanize-introduced one — guarded directly by
    /// `sections::is_usable_replacement`'s single-heading check and
    /// `sections::matches_requested_kind`'s identity check, which make a
    /// generated duplicate section rare rather than routine. The residual case
    /// (a user's own résumé, or an import, already carrying two sections of a
    /// kind) is pre-existing input-quality noise, not something this pipeline
    /// introduced, and a missed Warning there costs a lot less than a missed
    /// Critical.
    pub fn generated_sections_of_kind(&self, kind: SectionKind) -> impl Iterator<Item = &Section> {
        self.generated_sections
            .iter()
            .filter(move |s| s.kind == kind)
    }

    /// EVERY source section of `kind` — the SOURCE-side mirror of
    /// [`Self::generated_sections_of_kind`]. `factual::project_link_issues` used
    /// to read only the FIRST source section of a kind on this side while the
    /// generated side already read every one, so a SECOND source Projects
    /// section's links (`SECTION_NAMES` recognises both "projects" and "side
    /// projects", and both classify `Projects`) dropped out of the sourced set
    /// — a document that changed nothing accused itself of inventing its own
    /// link.
    pub fn source_sections_of_kind(&self, kind: SectionKind) -> impl Iterator<Item = &Section> {
        self.source_sections.iter().filter(move |s| s.kind == kind)
    }
}

/// Narrow any incoming language value to a 2-letter lowercase code, defaulting
/// to `"en"`. Mirrors `normalizeLanguageCode` in `natural-voice.ts`.
///
/// L-3 fix: filters to alphanumeric characters BEFORE taking the first 2 —
/// `.trim()` only strips LEADING/TRAILING whitespace, so a control character
/// in the middle (`"a\nb"`) used to survive into the 2-char result (`"a\n"`).
/// That result is interpolated into this module's `validate:content` span
/// text (`format!("kind={} lang={}", …)`) and becomes `ctx.lang`, which
/// reaches `content.language_mismatch`'s user-facing `evidence` — a raw
/// newline in either is a log-injection primitive (ADR-027-adjacent).
/// Filtering to alphanumeric also makes a tag like `"en-US"`/`"de_DE"`
/// resolve identically to before (the separator was never part of the first
/// 2 characters anyway).
pub(crate) fn normalize_language(language: &str) -> String {
    let code: String = language
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_lowercase();
    if code.is_empty() {
        "en".to_string()
    } else {
        code
    }
}

/// Characters that carry signal for language detection — everything but
/// whitespace. The one definition of "how much text is this really", shared by
/// [`language`]'s guards and [`Analysis::posting_language_diverges`], which
/// measure two different documents against the same bar.
pub(super) fn significant_chars(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

/// How many words a line may run to and still read as a HEADING rather than as
/// content. Four covers "Beruflicher Werdegang", "Weitere technische
/// Kenntnisse" and "Compétences techniques"; past that a line is a sentence.
const MAX_PROMOTED_HEADING_WORDS: usize = 4;

/// Whether a line `export::parser` did NOT classify as a heading should be read
/// as one anyway.
///
/// `parse_resume`'s heading test is an EXACT match against its own
/// `SECTION_NAMES` list, an ATX `#` marker, or ALL-CAPS. Between them those
/// cover a lot — every ALL-CAPS heading in any language, and the exact
/// single-word entries "Berufserfahrung", "Ausbildung", "Formation",
/// "Expérience professionnelle" — but they miss every Title-Case heading
/// OUTSIDE that exact list: "Beruflicher Werdegang", "Berufliche Erfahrung",
/// "Technische Kenntnisse", "Kurzprofil", "Compétences techniques".
/// `documents::evidence::classify_section` classifies all five correctly from
/// the shared multilingual heading lexicon, so the vocabulary is not missing —
/// only this module's access to it was.
///
/// What that cost is not cosmetic: a résumé whose headings are all Title-Case
/// collapses into ONE section, `factual::metric_lines` reads `has_headings` as
/// false, and the COVER-LETTER rules (the 8-word body latch) run over a résumé
/// — deleting every short source line, the candidate's own figures included,
/// which turns restating them into a fabrication Critical.
///
/// The lexicon alone is not enough to promote on, because it is a SUBSTRING
/// match built for text the parser already decided was a heading: "Improved
/// user experience by 20%" carries `experience` and "Cloud Kubernetes Docker"
/// carries `cloud`. So a promoted line must also LOOK like a heading, on
/// signals that hold in every language this pipeline supports:
///
/// * the parser left it as plain `Text`/`Name` — never a bullet, entry,
///   contact, job title or existing heading;
/// * it opens a block (first line, or preceded by a blank), which is where a
///   heading sits and where a line in the middle of a list does not;
/// * at most [`MAX_PROMOTED_HEADING_WORDS`] words and 60 characters;
/// * no digits and no sentence/column punctuation. The digit rule is
///   load-bearing beyond shape: a promoted line leaves the section's `lines`,
///   so this guarantees promotion can never remove a FIGURE from the source's
///   metric set, which is `factual::metric_lines`' second invariant.
fn reads_as_heading(line: &ParsedLine) -> bool {
    if !matches!(line.kind, LineKind::Text | LineKind::Name) {
        return false;
    }
    let text = line.text.trim();
    !text.is_empty()
        && text.chars().count() <= 60
        && word_count(text) <= MAX_PROMOTED_HEADING_WORDS
        && !text.chars().any(|c| c.is_ascii_digit())
        && !text.contains([
            '.', ',', ';', ':', '!', '?', '|', '·', '•', '@', '(', ')', '/',
        ])
        && classify_section(text) != SectionKind::Other
}

/// Whether the line DIRECTLY below a heading candidate opens an employment
/// entry — which makes the candidate that entry's job TITLE, not a heading.
///
/// ## The shape [`reads_as_heading`] cannot tell apart on its own
///
/// A job title on its own line above the employer is one of the two ordinary
/// experience layouts, and the extracted-PDF one:
///
/// ```text
/// BERUFSERFAHRUNG
///
/// Projektleiter                         ← the candidate
/// Acme Payments · Berlin · 2021 – Heute ← the entry it labels
/// - …
/// ```
///
/// Every shape guard passes: `export::parser`'s `JobTitle` arm needs the
/// PREVIOUS line to carry a two-space date column and this one is blank, so the
/// parser leaves it `Text`; it opens a block, because that is where an entry
/// block starts; it is one word, digit-free and punctuation-free. And
/// `classify_section` matches the `projekt`/`project` stem, so "Projektleiter",
/// "Project Manager", "Senior Project Manager" and "Technical Project Lead" all
/// became a `SectionKind::Projects` heading in the MIDDLE of the experience
/// section — which takes the entries below out of
/// [`factual::count_roles`]'s reach (a résumé reporting zero roles), grades their
/// bullets as malformed project cards (`consistency.project_structure`), and can
/// hand `factual::project_link_issues` a phantom projects section to compare the
/// source's real links against.
///
/// ## Why the line BELOW is the discriminator
///
/// A heading sits above a BLOCK; a title sits above a LINE. What follows a real
/// heading is a blank, a bullet, a stack line or a prose paragraph — what
/// follows a title-above-employer is the employer line itself. So the one signal
/// that separates them without a title vocabulary is: does the next line OPEN A
/// ROLE?
///
/// That question already has an owner. `documents::evidence::extract_evidence`
/// opens a role on exactly two shapes, and this reuses both rather than writing
/// a third opinion: a `LineKind::JobEntry` (the parser's two-space, pipe/middot
/// and parenthesized forms), or an unrecognised line ending in a real date
/// COLUMN ([`trailing_date_column`], which takes a column and not a mentioned
/// year — "Acme Payments, Berlin, 2018 - 2021"). A heading and an entry label
/// cannot drift apart about what an entry line is.
///
/// *Residual, stated:* an employer written across two lines with no date on the
/// first ("Projektleiter" / "Acme Payments" / "Berlin" / "2019 – 2021") still
/// promotes the title, because nothing on the line below says an entry started.
/// It costs a section split, not a false accusation.
fn labels_the_entry_below(next: Option<&ParsedLine>) -> bool {
    next.is_some_and(|line| {
        matches!(line.kind, LineKind::JobEntry) || trailing_date_column(&line.text).is_some()
    })
}

/// Split a document into sections at its headings. The leading band before the
/// first heading (name + contact) is always section 0 with `heading: None`, so
/// "is this in a non-first section?" is just an index test.
///
/// A line the parser did not recognise is promoted to a heading by
/// [`reads_as_heading`], **per line**.
///
/// ## Why the document-wide gate is gone
///
/// The promotion used to run only in a document where `export::parser` found NO
/// heading at all — the same "no better signal was available" rule
/// `documents::evidence`'s unclassified-section fallback uses. That reading
/// assumed the parser's `SECTION_NAMES` was English-only, and it is not: it
/// carries "ausbildung", "kenntnisse", "sprachen", "formation", "compétences"
/// and their siblings in seven locales. So ONE conventional single-word heading
/// ("Ausbildung") switched promotion off for the whole document — including for
/// this repair's own headline case, "Beruflicher Werdegang", in the completely
/// ordinary mixed résumé that heads three of its four sections in Title-Case and
/// the fourth in a word the list happens to hold. That document then reports the
/// Experience and Skills sections it visibly has as MISSING, and counts none of
/// its roles.
///
/// ## What defends the promotion instead
///
/// The risk the gate was covering is an ordinary prose line resembling a heading
/// in the MIDDLE of a well-headed document, which would split a role in half. It
/// is covered by [`reads_as_heading`]'s own shape guards, which is where a
/// statement about what a heading LOOKS like belongs: the classifier must
/// recognise the line (a generic shape is never promoted), the parser must have
/// left it as plain `Text`/`Name`, it must open a block, and it must carry no
/// digits and no sentence/column punctuation. A stack line or a bullet
/// continuation fails the block test; a sentence fails the length and
/// punctuation tests; and the digit rule still guarantees promotion cannot
/// remove a FIGURE from the source's metric set.
///
/// …and it must not be the LABEL of the entry underneath it
/// ([`labels_the_entry_below`], which is what keeps an ordinary job title above
/// its employer out of the heading list).
///
/// *Residual, stated:* a ≤4-word, digit-free, punctuation-free `Text` line that
/// opens a block, carries a heading stem ("Cloud Kubernetes Docker" would, at
/// four words) and is not followed by an entry line is promoted wherever it
/// sits. That was already accepted for a heading-less document; it is the same
/// error, now reachable in a headed one, and it costs a section split rather
/// than a false accusation.
///
/// ## Promotion is a RÉSUMÉ repair — `doc_kind` decides, per text
///
/// Everything above is an argument about documents that HAVE sections. A cover
/// letter has none, so `export::parser` never finds a heading in one and every
/// short label line in it — "My Experience", "Kurzprofil", "Zu meiner Person" —
/// satisfies every shape guard there is. One of them is enough to take
/// `factual::metric_lines`' `sections.len() > 1` test from false to true, which
/// switches that pass from the LETTER rules to the résumé ones: section 0 —
/// everything above the label, i.e. most of the letter — is then skipped by
/// POSITION on the claims side, and the numbers in the letter's opening stop
/// being checked against the source at all. Silencing rather than accusing, but
/// structural: the letter loses the check the whole family exists for.
///
/// The parameter is the kind of THIS TEXT, not of the report, which is why it is
/// threaded rather than read off `ContentInput`: when the report is validating a
/// LETTER, the source résumé it is measured against is still a résumé and still
/// needs the repair.
pub(crate) fn split_sections(text: &str, doc_kind: DocKind) -> Vec<Section> {
    let lines = parse_resume(text).lines;
    let mut sections = vec![Section {
        heading: None,
        kind: SectionKind::Other,
        lines: Vec::new(),
    }];
    let mut opens_a_block = true;
    let mut lines = lines.into_iter().peekable();
    while let Some(line) = lines.next() {
        let is_heading = matches!(line.kind, LineKind::SectionHeader)
            || (doc_kind == DocKind::Resume
                && opens_a_block
                && reads_as_heading(&line)
                && !labels_the_entry_below(lines.peek()));
        opens_a_block = matches!(line.kind, LineKind::Blank);
        if is_heading {
            sections.push(Section {
                kind: classify_section(&line.text),
                heading: Some(line.text.clone()),
                lines: Vec::new(),
            });
        } else if let Some(current) = sections.last_mut() {
            current.lines.push(line);
        }
    }
    sections
}
