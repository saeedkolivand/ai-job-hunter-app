//! Shared large résumé fixture strings reused across the typst_engine test topics.

/// Short one-page resume fixture — enough content to exercise all block types
/// (header, paragraph, entry with bullets, standalone bullets) while keeping
/// compilation fast.
pub(super) const FIXTURE_RESUME: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe | https://github.com/janedoe

SUMMARY
Experienced software engineer with a passion for building reliable systems.

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Designed distributed task scheduler reducing latency by 40 percent
- Led migration to Rust-based microservices across three product teams

Software Engineer | Beta Inc | 2018 – 2021
- Built real-time data pipeline processing one million events per day
- Mentored two junior engineers through onboarding

EDUCATION
B.Sc. Computer Science | State University | 2014 – 2018

SKILLS
Rust, Python, TypeScript, PostgreSQL, Kubernetes, AWS
";

/// Accented-Latin résumé fixture — grave-accented lowercase (à, ò, ì) PLUS
/// capital grave accents (È, À), the less-tested shape flagged by the
/// `no_extractable_text` incident audit (a macOS user could not download
/// either PDF; the leading hypothesis is a broken ToUnicode CMap on a subset
/// font — glyphs render fine on screen but `pdf_extract` gets nothing back).
/// The incident involved Italian text; German (ü) is already covered by the
/// DE letter fixtures and the Portrait/Saffron "Über Ödegaard" grapheme pins.
/// Same section-heading shape as [`FIXTURE_RESUME`] so section classification
/// is unaffected — only the header name and body prose carry accents.
pub(super) const ACCENTED_RESUME_FIXTURE: &str = "\
Àlvaro Èsposito
alvaro.esposito@example.it | https://linkedin.com/in/alvaroesposito

SUMMARY
Ingegnere del software cresciuto vicino a Città di Torino, però orientato ai \
sistemi distribuiti costruiti così da scalare senza sforzo.

EXPERIENCE
Senior Engineer | Acme Corp | 2021 – Present
- Migrated the payments service to a microservices architecture, cutting latency by 40 percent
- Guidato il team attraverso la migrazione, mantenendo però sempre alta la qualità

EDUCATION
Laurea in Informatica | Università degli Studi di Torino | 2014 – 2018

SKILLS
Rust, Python, TypeScript, PostgreSQL, Kubernetes, AWS
";

//
// Tests cover:
//   (1) Basic render — valid PDF in both ats:false and ats:true.
//   (2) 2-page sidebar repeat — enough content to force ≥2 pages; ALL sidebar
//       items from the fixture must be present in the extracted text (regression
//       guard for the dense-sidebar overflow fix, F1/F4).
//   (3) ATS collapse — ats:true → linear reading order, sidebar headings appear
//       AFTER the main-column headings but still present and in order.
//   (4) Entry integrity — titles + bullets present in extracted text.
//   (5) Accent override — custom accent does not cause a compile error.
//   (6) Sample PDF written to target/ for human review (informational, always passes).
//   (7) Dense-sidebar fixture — 10+ skills, 2 degrees, 3 certs, 4 languages;
//       every sidebar item must be present (F1 regression guard).
//   (8) Empty-sidebar fixture — all sections placed in main; no sidebar sections;
//       template must fall back to single-column (no band) and render cleanly.

/// Single-page fixture — exercises all block types.
pub(super) const ATELIER_FIXTURE: &str = "\
Alexandra Rivera
alex@example.com | [LinkedIn](https://linkedin.com/in/alexrivera) | https://alexrivera.dev

SUMMARY
Product-focused engineering leader with twelve years building distributed systems.

EXPERIENCE
Principal Engineer | Meridian Systems | 2019 – Present
- Scaled the event-sourcing platform to 500 k events per second
- Drove adoption of a domain-driven architecture across seven product teams

Software Engineer | Cobalt Labs | 2015 – 2019
- Built the real-time collaboration layer used by 200 k active users
- Reduced cold-start latency from 900 ms to 110 ms

EDUCATION
M.Sc. Computer Science | Western University | 2013 – 2015

SKILLS
Rust, Go, TypeScript, Kubernetes, AWS, Kafka, PostgreSQL

LANGUAGES
English (native), Portuguese (fluent)
";

/// Multi-page fixture — enough experience + project entries to force ≥2 pages.
/// The main-column content (SUMMARY + EXPERIENCE + PROJECTS) is deliberately
/// long enough to overflow a single A4 page in the 70% main column.
pub(super) const ATELIER_MULTIPAGE: &str = "\
Alexandra Rivera
alex@example.com | https://alexrivera.dev

SUMMARY
Engineering leader with a decade of distributed-systems experience building resilient
platforms at scale. Passionate about developer productivity, reliability engineering,
and growing high-performing teams across multiple time zones.

EXPERIENCE
Staff Engineer | Apex Corp | 2022 – Present
- Led the platform-reliability initiative that reduced P99 latency by 60 percent across all production services
- Introduced chaos engineering practices that were adopted across twelve service teams globally
- Architected a zero-downtime schema migration pipeline managing a 10 TB customer dataset
- Mentored eight engineers through promotion to senior level over the course of eighteen months
- Drove the company-wide observability strategy resulting in 99.99 percent annual SLA achievement
- Defined engineering excellence standards that were subsequently adopted by all thirty backend teams
- Designed the on-call runbook system reducing mean time to resolution from 45 minutes to 8 minutes

Senior Engineer | Meridian Systems | 2019 – 2022
- Built the multi-tenant billing engine that processed 50 M transactions per month without downtime
- Migrated a legacy monolith to fifty domain-aligned microservices over an eighteen-month programme
- Designed the event-sourcing backbone now serving 300 k events per second at peak production load
- Reduced infrastructure cost by 35 percent through adaptive auto-scaling policies and spot instances
- Shipped a real-time analytics dashboard that was adopted by over 10 k business users on launch day
- Onboarded and technically led a distributed team of nine engineers across three time zones

Software Engineer | Cobalt Labs | 2016 – 2019
- Delivered the real-time collaboration layer for the flagship product used by 200 k daily active users
- Implemented end-to-end encryption for all user-generated content at rest and in transit
- Reduced cold-start API latency from 900 ms to 110 ms through optimised connection pooling strategies
- Contributed core modules to three open-source libraries with a combined 8 k GitHub stars

Junior Software Engineer | Vertex Startup | 2014 – 2016
- Shipped the initial iOS client that reached 50 k downloads in the first month after public launch
- Rebuilt the search indexing pipeline and cut ingestion lag from five minutes to eight seconds
- Integrated third-party payment providers handling 500 k transactions per day in a PCI-DSS environment

PROJECTS
Distributed Rate Limiter | Open Source | 2021
- Designed a Redis-backed token-bucket rate limiter with sub-millisecond overhead per request
- Published to crates.io; adopted by fourteen organisations within six months of initial release
- Maintained comprehensive documentation, changelog, and semver-stable public API

High-Throughput Log Aggregator | Open Source | 2020
- Built a lock-free ring-buffer pipeline aggregating 1 M log lines per second on commodity hardware
- Presented at a regional systems-programming conference to an audience of 400 engineers

EDUCATION
M.Sc. Computer Science | Western University | 2012 – 2014
B.Sc. Computer Engineering | Eastern College | 2008 – 2012

SKILLS
Rust, Go, TypeScript, Kubernetes, AWS, GCP, Kafka, PostgreSQL, Redis, Terraform, Prometheus, Grafana

LANGUAGES
English (native), Portuguese (fluent), Spanish (working)
";

/// Dense-sidebar fixture — 10+ skills, 2 degrees, 3 certifications, 4 languages.
/// This is the F1 regression fixture: the sidebar content is tall enough that
/// the template must detect overflow and fall back to single-column so that
/// no sidebar item is silently clipped.
pub(super) const ATELIER_DENSE_SIDEBAR: &str = "\
Jordan Kim
jordan@example.com | https://linkedin.com/in/jordankim | https://jordankim.dev

SUMMARY
Polyglot engineer with deep expertise in distributed systems and cloud infrastructure.

EXPERIENCE
Senior Platform Engineer | Globex Corp | 2020 – Present
- Designed a multi-region failover system achieving five nines availability
- Reduced mean deployment time from 45 minutes to under four minutes

Platform Engineer | Initech Solutions | 2017 – 2020
- Built a shared CI/CD platform adopted by 80 engineering teams
- Introduced contract testing reducing integration failures by 70 percent

EDUCATION
M.Eng. Software Engineering | Metro University | 2015 – 2017
B.Sc. Computer Science | Coastal College | 2011 – 2015

SKILLS
Rust, Go, Python, TypeScript, Java, Kotlin, C++, Bash, SQL, Terraform, Ansible, Pulumi

LANGUAGES
English (native), German (fluent), French (professional), Mandarin (conversational)

CERTIFICATIONS
AWS Solutions Architect Professional
Google Cloud Professional Data Engineer
Certified Kubernetes Administrator
";

/// German Lebenslauf fixture — uses typical DACH names and section content.
pub(super) const LEBENSLAUF_FIXTURE: &str = "\
Max Müller
max.mueller@example.de | https://linkedin.com/in/maxmueller

BERUFSERFAHRUNG
Senior Software Engineer | Musterfirma GmbH | 2020 – Heute
- Entwicklung einer hochverfügbaren Microservices-Architektur mit Kubernetes
- Einführung von CI/CD-Pipelines und Reduktion der Deployment-Zeit um 60 Prozent

Software Engineer | Tech AG | 2017 – 2020
- Aufbau einer Echtzeit-Datenplattform für zwei Millionen tägliche Nutzer
- Mentoring von drei Junior-Entwicklern im Bereich Rust und TypeScript

AUSBILDUNG
M.Sc. Informatik | Technische Universität Berlin | 2015 – 2017

KENNTNISSE
Rust, Go, TypeScript, Kubernetes, AWS, PostgreSQL, Kafka

SPRACHEN
Deutsch (Muttersprache), Englisch (fließend)
";

//
// Both are photo-capable two-column templates rendered through bespoke `.typ`
// sources.  Per template we assert: valid PDF with + without a photo (fallback
// path), ATS mode drops the photo (SVG `<image>` assert like Lebenslauf), the
// document-accent override changes the output, `is_two_column` is true, a 2-page
// fixture keeps the sidebar band to page 1, and the per-template placement
// override lands the moved section in the main column.

/// Fixture with distinct EDUCATION + CERTIFICATIONS + SKILLS sections so the
/// per-template placement override can be asserted at the serialized-JSON level.
pub(super) const PLACEMENT_FIXTURE: &str = "\
Jane Doe
jane@example.com | https://linkedin.com/in/janedoe

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Built a distributed task scheduler

EDUCATION
State University  2013 - 2017
BSc Computer Science

SKILLS
- Rust, Go, TypeScript

CERTIFICATIONS
- AWS Certified Solutions Architect
";

//
// Renders all twelve templates, rasterises the first page of each at 2× DPI
// (144 px/pt), thumbnails each to 300 px wide, and composes a single wide
// row (1×12) — a banner-proportioned strip like the project hero — on a
// #F4F4F5 background with 20 px border-padding and 14 px gaps, writing the
// result to docs/assets/templates-showcase.png.
//
// That path is GITIGNORED and must stay that way: the banner is ~1.4 MB of
// render output the README embeds, so it is served from the parentless `assets`
// branch rather than committed here. After regenerating, publish it with the
// snippet in docs/EXPORT_TEMPLATES.md; do not `git add -f` it.
//
// As a side output it also writes one per-template preview SVG to
// apps/desktop/src/renderer/features/ai-generate/assets/template-previews/<id>.svg,
// which the AI-Generate option previews show in the result panel. SVG (vector)
// replaces the old PNGs — crisp at any zoom and a fraction of the bundle size.
//
// This test is `#[ignore]`d so it never runs in the normal CI suite.
// Run it explicitly with (the crate is a binary, so target the bin, not --lib):
//   cargo test --bin ajh-tauri -- --ignored generate_templates_showcase_banner
//
// No personal data — synthetic fixture only.  No text-caption rendering dep.

/// Full showcase fixture — richer than FIXTURE_RESUME so templates show premium
/// styling: summary paragraph, multi-entry experience with bullets, skills,
/// education, languages.  Synthetic identity (Alex Carter, example.com contacts).
pub(super) const SHOWCASE_FIXTURE: &str = "\
Alex Carter
alex.carter@example.com | https://linkedin.com/in/alexcarter | https://alexcarter.dev

SUMMARY
Versatile engineering leader with ten years building high-performance distributed
systems across fintech, healthcare, and cloud infrastructure. Known for bridging
deep technical expertise with product intuition to ship reliable platforms at scale.

EXPERIENCE
Staff Engineer | Apex Technologies | 2021 – Present
- Designed a multi-region event-sourcing platform processing 800 k events per second
- Led architectural review programme adopted by forty backend teams company-wide
- Reduced P99 API latency from 420 ms to 18 ms through adaptive connection pooling
- Mentored six engineers to senior level; two subsequently promoted to staff

Senior Engineer | Meridian Cloud | 2018 – 2021
- Built a zero-downtime schema-migration pipeline managing a 12 TB customer dataset
- Delivered the real-time collaboration layer used by 350 k daily active users
- Cut infrastructure spend by 38 percent via spot-instance scheduling and auto-scaling
- Shipped an internal observability platform reducing mean time-to-resolve by 70 percent

Software Engineer | Cobalt Labs | 2015 – 2018
- Implemented end-to-end encryption for all user-generated content at rest and in transit
- Rebuilt the search-indexing pipeline; ingestion lag dropped from six minutes to nine seconds
- Contributed core modules to four open-source libraries with a combined 12 k GitHub stars

PROJECTS
Distributed Rate Limiter | Open Source | 2022
- Redis-backed token-bucket rate limiter with sub-millisecond overhead per request
- Published on crates.io; adopted by twenty organisations within four months of launch

EDUCATION
M.Sc. Computer Science | Westbrook University | 2013 – 2015
B.Sc. Software Engineering | Coastal College | 2009 – 2013

SKILLS
Rust, Go, TypeScript, Python, Kubernetes, AWS, GCP, Kafka, PostgreSQL, Redis, Terraform

LANGUAGES
English (native), Spanish (professional), German (conversational)

CERTIFICATIONS
AWS Solutions Architect Professional
Certified Kubernetes Administrator
";
