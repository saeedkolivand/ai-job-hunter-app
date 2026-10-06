// ── 1. Corpus ────────────────────────────────────────────────────────────────

/**
 * REAL FIXTURE — verbatim content of
 * `apps/desktop/src-tauri/tests/fixtures/resume.txt`.
 *
 * This is the canonical Rust-test fixture; using it here proves the TS
 * serializer agrees with what the Rust parser already accepts. Known-section
 * headings (Summary, Experience, Education, Skills, Languages, Certifications),
 * a name+contact header, and job entries in form (b) `Role — Company (date)`.
 */
export const CORPUS_REAL_RESUME = `Jane Doe
jane.doe@example.com | +31 6 12345678 | linkedin.com/in/janedoe

Summary
Experienced software engineer with 8 years building distributed systems in Rust and Go.

Experience
Senior Software Engineer — Acme Corp, Amsterdam (2020–2025)
- Led migration of monolith to microservices, reducing p99 latency by 40%.
- Mentored team of 5 engineers; introduced weekly architecture reviews.

Software Engineer — Startup B.V., Rotterdam (2017–2020)
- Built real-time data pipeline processing 50k events/sec using Kafka and Rust.

Education
BSc Computer Science — University of Amsterdam (2013–2017)

Skills
Rust, Go, TypeScript, Python, PostgreSQL, Kafka, Docker, Kubernetes

Languages
English (fluent), Dutch (intermediate), German (basic)

Certifications
AWS Solutions Architect — Associate (2022)`;

/**
 * Crafted sample A — resume with a trailing link-reference block.
 *
 * Covers:
 * - name + contact header (pipe-separated with inline link)
 * - known section names (Summary, Experience, Education, Skills)
 * - ALL-CAPS banner heading (PROFESSIONAL EXPERIENCE)
 * - custom markdown heading (## Side Projects, ### Open Source)
 * - job-entry form (a): 2+ literal spaces before date (THE double-space risk)
 * - job-entry form (b): trailing parenthesized date
 * - job-entry form (c): pipe/middot-separated
 * - flat bullet list
 * - inline bold, italic, link on a contact line
 * - trailing \n---\n link-reference block (held out by splitPreserved)
 */
export const CORPUS_RESUME_WITH_LINK_BLOCK = `Alex Kim
alex.kim@example.com | [LinkedIn](https://linkedin.com/in/alexkim) | [GitHub](https://github.com/alexkim)

Summary
Full-stack engineer with **10 years** of experience in *distributed systems* and cloud infrastructure.

PROFESSIONAL EXPERIENCE

## Experience

Senior Staff Engineer  Stripe, San Francisco  Jan 2021 – Present
- Designed the payment orchestration layer handling $2B/day in transaction volume.
- Reduced fraud rate by 18% using *real-time* ML scoring pipeline.
- Led a team of **8 engineers** across 3 time zones.

Staff Engineer, Cloudflare (Mar 2018 – Dec 2020)
- Built [Workers KV](https://developers.cloudflare.com/workers/runtime-apis/kv/) storage layer.
- Achieved 99.99% uptime across 200+ edge locations.

Principal Engineer | Acme Corp | 2015 – 2018
- Migrated legacy monolith to microservices; reduced deploy time from 4h to 12m.

## Side Projects

### Open Source
- [rust-http-client](https://github.com/alexkim/rust-http): async Rust HTTP client (2k stars).
- Contributed to **tokio** runtime: 5 merged PRs.

Education
MSc Computer Science — MIT (2013–2015)
BSc Computer Science — UC Berkeley (2009–2013)

Skills
Rust, Go, TypeScript, Python, PostgreSQL, Redis, Kafka, Kubernetes, Terraform

Languages
English (native), Korean (fluent)
\n---\n- [LinkedIn](https://linkedin.com/in/alexkim)\n- [GitHub](https://github.com/alexkim)`;

/**
 * Crafted sample B — cover letter with a link block.
 *
 * Covers:
 * - prose paragraphs (multi-sentence, no headings initially)
 * - inline bold + italic in body text
 * - literal `·` and `|` and `(` `)` in body that MUST NOT be escaped
 * - a trailing link-reference block
 */
export const CORPUS_COVER_LETTER_WITH_LINK_BLOCK = `Maria Santos
maria.santos@example.com · +49 30 12345678 · [Portfolio](https://mariasantos.dev)

Dear Hiring Manager,

I am writing to apply for the **Senior Product Designer** position at Figma. With *7 years* of experience designing (and shipping) complex B2B interfaces, I believe I am an excellent fit.

My work at Craft (2019–2023) focused on design systems — I built and maintained a component library used by 40+ product teams. The system reduced designer-to-developer handoff time by 60%.

Prior to that, at Pixel Studio (2016–2019), I led end-to-end UX for three flagship products, each with 1M+ active users. One product won the Red Dot Design Award | Product Design | 2021.

I thrive in fast-moving environments and enjoy the intersection of strategy · execution · craft. I would love to discuss how my background aligns with Figma's goals.

Sincerely,
Maria Santos
\n---\n- [Portfolio](https://mariasantos.dev)\n- [LinkedIn](https://linkedin.com/in/mariasantos)`;

/**
 * Crafted sample C — all three job-entry date forms in one document.
 *
 * This is the PRIMARY risk sample. Each form must survive BYTE-EXACT.
 *   (a) 2+ literal spaces: `Senior Engineer  Acme Corp  Jan 2020 – Present`
 *   (b) trailing parens:   `Staff Engineer, Contoso (Mar 2018 – Dec 2019)`
 *   (c) pipe-separated:    `Junior Engineer | Widget Co | 2016 – 2018`
 */
export const CORPUS_ALL_DATE_FORMS = `Jordan Lee
jordan.lee@example.com | GitHub | LinkedIn

Experience

Senior Engineer  Acme Corp  Jan 2020 – Present
- Designed distributed caching layer; reduced DB load by 35%.
- Shipped **4 major features** in 18 months with *zero* regressions.

Staff Engineer, Contoso (Mar 2018 – Dec 2019)
- Led rewrite of legacy billing system from PHP to Go.
- Reduced monthly invoice processing time from 6h to 45m.

Junior Engineer | Widget Co | 2016 – 2018
- Built REST API for internal tooling · 50k requests/day.
- Automated deployment pipeline (Jenkins → GitHub Actions).

Education
BSc Software Engineering — Stanford University (2012–2016)

Skills
Go, TypeScript, PostgreSQL, Redis, Kubernetes`;

/**
 * Crafted sample D — markdown headings in all three detectable forms,
 * consecutive blank lines as block separators, H3 subheadings.
 */
export const CORPUS_HEADING_VARIANTS = `Sam Rivera
sam.rivera@example.com | +1 555 0100

## Summary
Senior engineer with a passion for *open source* and **systems programming**.

PROFESSIONAL EXPERIENCE

Senior Engineer  Mozilla Foundation  2021 – Present
- Worked on *Firefox* performance; reduced startup time by 20%.

## Side Projects

### Rust Projects
- [ferrocene](https://ferrocene.dev): Rust for safety-critical systems (contributor).
- rust-analyzer extensions for better **lifetime** visualization.

### Web Projects
- Built a [real-time collab editor](https://example.com/editor) using CRDTs.

Skills
Rust, C++, TypeScript, WebAssembly, Linux

Education
BSc Computer Science — University of Toronto (2015–2019)`;

// The corpus map drives the no-drift gate parametrically.
export const CORPUS: Record<string, string> = {
  'real fixture: resume.txt (known sections + contact header + form-b dates)': CORPUS_REAL_RESUME,
  'full resume: all three date forms in realistic context': CORPUS_ALL_DATE_FORMS,
  'full resume: with trailing link-reference block': CORPUS_RESUME_WITH_LINK_BLOCK,
  'cover letter: with trailing link-reference block': CORPUS_COVER_LETTER_WITH_LINK_BLOCK,
  'full resume: heading variants (custom + ALL-CAPS + H3)': CORPUS_HEADING_VARIANTS,
};
