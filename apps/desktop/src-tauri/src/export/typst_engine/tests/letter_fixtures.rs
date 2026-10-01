//! Shared cover-letter fixture strings reused across the typst_engine test topics.

//
// Tests cover:
//   (1) US letter — renders valid PDF on Letter-size page (215.9 × 279.4 mm),
//       no subject line; salutation, body phrase, sign-off present.
//   (2) DE letter — renders valid PDF on A4; DIN subject "Betreff:" present;
//       German salutation + signoff recognised.
//   (3) Both PDFs start with %PDF.
//   (4) Sample PDF writers: target/letter_us_sample.pdf and
//       target/letter_de_sample.pdf — informational, always pass.

/// US English cover letter fixture.
pub(super) const LETTER_FIXTURE_US: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. With five years of experience building distributed systems in Rust and \
Go, I believe I would be a great addition to your team.

During my time at Beta Inc, I led the migration of our payments service to a \
microservices architecture, reducing end-to-end latency by 40 percent and \
cutting infrastructure costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with your needs.

Sincerely,

Jane Smith
Software Engineer
";

/// German DIN 5008 cover letter fixture.
pub(super) const LETTER_FIXTURE_DE: &str = "\
Max Müller
max@example.de | https://linkedin.com/in/maxmueller

Frankfurt, 2. Juni 2025

Frau Dr. Anna Weber
Musterfirma GmbH
Hauptstraße 1
60311 Frankfurt am Main

Betreff: Bewerbung als Software Engineer

Sehr geehrte Frau Dr. Weber,

mit großem Interesse habe ich Ihre Stellenausschreibung für die Position als \
Software Engineer gelesen. Ich bewerbe mich hiermit für diese Stelle.

In meiner bisherigen Tätigkeit bei der Beta GmbH habe ich umfangreiche Erfahrungen \
in der Entwicklung verteilter Systeme gesammelt und konnte die Systemlatenz um \
40 Prozent reduzieren.

Über eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen.

Mit freundlichen Grüßen,

Max Müller
";

/// Accented-Latin cover-letter fixture — grave-accented lowercase (à, ò, ì)
/// PLUS capital grave accents (È, À), the shape the `no_extractable_text`
/// incident audit flagged as under-tested (see [`ACCENTED_RESUME_FIXTURE`]
/// for the full rationale). US-market shape (reuses [`LETTER_FIXTURE_US`]'s
/// structure) so DIN-specific parsing isn't a confound here.
pub(super) const LETTER_FIXTURE_IT: &str = "\
Àlvaro Èsposito
alvaro.esposito@example.it | https://linkedin.com/in/alvaroesposito

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street
New York, NY 10001

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. Growing up near Città di Torino and studying at the Università degli \
Studi, I built a solid foundation in distributed systems — però my passion has \
always been building things that scale così well they disappear into the \
background.

During my five years at Beta Inc, I led the migration of our payments service to \
a microservices architecture, reducing end-to-end latency by 40 percent.

Sincerely,

Àlvaro Èsposito
Software Engineer
";

//
// `pipeline::resume::prompts::letter_system` instructs the model: "Do NOT
// write a contact header, a salutation line, or a signature block — the
// application adds them at export time." For months the application did
// not: `export::letter_shape::complete_letter_text` did not exist, so the
// pipeline's staged letter output never got its furniture. Every letter
// fixture ABOVE this point (`LETTER_FIXTURE_US`/`_DE`/`_IT`) is the COMPLETE
// shape the OLD prompt produced — full letterhead, salutation and sign-off
// already present — so not one of them ever exercised the completion path.
// These two are body-only: three plain paragraphs, no letterhead, no
// salutation, no sign-off, no signature — exactly what the CURRENT prompt
// asks for. See `body_only_us_letter_gets_completed_furniture_in_the_pdf_text_layer`
// / `body_only_de_letter_gets_completed_furniture_in_the_pdf_text_layer` near
// the end of this file for the guardrail tests that render them.

/// US body-only fixture. One `**bold**` keyword — bold is lost the instant a
/// paragraph is misrouted into the plain-text recipient block (what happens
/// pre-fix, since with no salutation `parse_cover_letter` never flips
/// `body_started` and every paragraph falls into the pre-salutation
/// classification branch), so it is a precise detector of that failure.
pub(super) const LETTER_FIXTURE_BODY_ONLY_US: &str = "\
I am writing to express my strong interest in the Software Engineer position, \
where I would bring five years of experience building distributed systems in \
Rust and Go to a team solving problems at real scale.

During my time at Beta Inc, I led the migration of our payments service to a \
**microservices** architecture, reducing end-to-end latency by 40 percent and \
cutting infrastructure costs by 30 percent.

I would welcome the opportunity to discuss how my background aligns with \
your team's needs and how I could contribute from day one.";

/// German body-only fixture — adversarial in the three ways the real letter
/// was: a long opening paragraph (the shape that used to render as the
/// letterhead name — the day-one bug's most visible symptom), a paragraph
/// containing digits and a mid-sentence period ("von 0 % auf 90 %.", the
/// shape `looks_like_date` (`typst_engine/letterhead.rs::looks_like_date`)
/// used to mis-classify as the date), and a `**bold**` keyword (lost the
/// instant a paragraph is misrouted into the plain-text recipient block).
pub(super) const LETTER_FIXTURE_BODY_ONLY_DE: &str = "\
Mit großem Interesse habe ich Ihre Stellenausschreibung für die Position als \
Software Engineer gelesen und bin überzeugt, dass meine mehrjährige \
Erfahrung in der Entwicklung verteilter Systeme genau zu den Anforderungen \
passt, die Sie beschrieben haben.

In meiner bisherigen Tätigkeit bei der Beta GmbH konnte ich die \
Testabdeckung von 0 % auf 90 % steigern. Durch die Einführung von **Jest** \
und einer durchgängigen CI-Pipeline wurde die Codequalität spürbar besser.

Über eine Einladung zum Vorstellungsgespräch würde ich mich sehr freuen und \
stehe für Rückfragen jederzeit zur Verfügung.";

//
// The three layouts share the identical `LetterModel` / `data.json` contract;
// only the arrangement (`letter*.typ` source) differs. Palette + fonts still
// inherit from the résumé template, and market conventions (DE DIN date-top-
// right + subject line, US below-header) are still honoured per layout.

/// US letter carrying an explicit "Re: …" subject line. The US market sets
/// `subject_line_used = false`, so the Classic layout drops this subject while
/// the Refined layout always foregrounds it — the discriminator below.
pub(super) const LETTER_FIXTURE_US_SUBJECT: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp
123 Main Street

Re: Application for Platform Engineer (Ref PX-2291)

Dear Hiring Manager,

I am writing to express my strong interest in the Platform Engineer position at \
Acme Corp, where I would bring five years of distributed-systems experience.

Sincerely,

Jane Smith
Software Engineer
";

/// A long US letter that reflows onto multiple pages — used to prove the Banded
/// layout draws its accent band on page 1 only.
pub(super) const LETTER_FIXTURE_LONG_US: &str = "\
Jane Smith
jane@example.com | https://linkedin.com/in/janesmith

June 2, 2025

Hiring Manager
Acme Corp

Dear Hiring Manager,

I am writing to express my strong interest in the Software Engineer position at \
Acme Corp. Over the past five years I have designed and operated distributed \
systems in Rust and Go, consistently reducing latency and cost while raising the \
reliability bar for every team I have worked with.

During my time at Beta Inc I led the migration of our payments service to a \
microservices architecture, reducing end-to-end latency by 40 percent and \
cutting infrastructure costs by 30 percent across the platform.

I introduced a service-level-objective culture, instrumented the critical paths, \
and mentored a cohort of engineers who now own those services end to end. The \
result was a measurable drop in incidents and a faster, calmer on-call rotation.

At Gamma LLC I rebuilt the ingestion pipeline to handle a tenfold increase in \
event volume without a proportional increase in cost, using back-pressure and \
adaptive batching to keep tail latency predictable under bursty load.

I care deeply about developer experience, and I have shipped internal tooling \
that shortened the local feedback loop from minutes to seconds, which paid for \
itself many times over in team velocity and morale.

Beyond the technical work, I have partnered closely with product and design to \
make sure the systems we build actually serve the people who use them, and I \
have found that this partnership consistently produces better outcomes.

I would welcome the opportunity to bring the same rigour, curiosity, and sense \
of ownership to your team, and to help you scale the platform through its next \
phase of growth with confidence and care.

Earlier in my career at Delta Systems I built the on-call tooling that our whole \
engineering org still relies on, cutting mean time to resolution significantly \
by surfacing the right context the moment an alert fired.

I have also invested heavily in testing culture, introducing contract tests \
between services that caught integration regressions before they ever reached \
production, which meaningfully reduced the number of incidents quarter over \
quarter.

Outside of pure execution, I enjoy mentoring engineers earlier in their careers, \
pairing regularly and helping them build the judgment to make good trade-offs \
under real constraints rather than just following a checklist.

I have led cross-team initiatives that required aligning stakeholders with \
different priorities, and I take real pride in finding the solution that \
actually satisfies everyone's constraints rather than the loudest one.

Reliability work is often invisible when done well, so I have made a habit of \
writing clear postmortems and sharing them broadly, turning painful incidents \
into lasting organizational learning rather than one-off fire drills.

Thank you for considering my application; I would be glad to discuss how my \
background aligns with your needs at any time that is convenient for you.

Sincerely,

Jane Smith
Software Engineer
";
