/**
 * Hard no-drift idempotency gate + targeted edit tests for the WYSIWYG
 * markdown round-trip (§ Verification in the WYSIWYG plan).
 *
 * REQUIREMENT (plan §C): serialize(parse(md)) === md byte-exact for every
 * unedited real document. Any failure here is a BUG in markdown.ts, not in
 * this file — do NOT weaken assertions to make a broken round-trip pass.
 *
 * Sections:
 *   1. Corpus fixtures — real + crafted full-document samples
 *   2. Hard no-drift gate — roundTrip(md) === md for every corpus entry
 *   3. splitPreserved / joinPreserved direct unit assertions
 *   4. Targeted edit tests — one programmatic PM change; rest unchanged
 *   5. Edge / negative cases
 */

import { describe, expect, it } from 'vitest';

import {
  docToMarkdown,
  getEditorSchema,
  joinPreserved,
  markdownToDoc,
  roundTrip,
  splitPreserved,
} from './markdown';

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
const CORPUS: Record<string, string> = {
  'real fixture: resume.txt (known sections + contact header + form-b dates)': CORPUS_REAL_RESUME,
  'full resume: all three date forms in realistic context': CORPUS_ALL_DATE_FORMS,
  'full resume: with trailing link-reference block': CORPUS_RESUME_WITH_LINK_BLOCK,
  'cover letter: with trailing link-reference block': CORPUS_COVER_LETTER_WITH_LINK_BLOCK,
  'full resume: heading variants (custom + ALL-CAPS + H3)': CORPUS_HEADING_VARIANTS,
};

// ── 2. Hard no-drift gate ────────────────────────────────────────────────────

describe('no-drift gate: roundTrip(md) === md byte-exact (full corpus)', () => {
  for (const [label, md] of Object.entries(CORPUS)) {
    it(label, () => {
      const result = roundTrip(md);
      // Strong assertion: exact string equality — no normalization tolerance.
      // If this fails, report actual vs expected in the assertion message so
      // the orchestrator can route a targeted fix to markdown.ts.
      expect(result).toBe(md);
    });
  }

  // Individual coverage assertions for the plan's explicit must-survive list.

  it('double-space job-entry form (a): 2+ spaces before date survive byte-exact', () => {
    // This is the headline risk: markdown parsers collapse runs of spaces.
    // The custom line-oriented parser must NOT do that.
    const md = 'Senior Engineer  Acme Corp  Jan 2020 – Present';
    expect(roundTrip(md)).toBe(md);
    // Verify both the 2-space AND 4-space variants.
    const md4 = 'Staff Engineer    Cloudflare    Mar 2018 – Dec 2020';
    expect(roundTrip(md4)).toBe(md4);
  });

  it('job-entry form (b): trailing parenthesized date survives byte-exact', () => {
    const md = 'Staff Engineer, Contoso (Mar 2018 – Dec 2019)';
    expect(roundTrip(md)).toBe(md);
  });

  it('job-entry form (c): pipe/middot-separated entry survives byte-exact', () => {
    const pipe = 'Junior Engineer | Widget Co | 2016 – 2018';
    expect(roundTrip(pipe)).toBe(pipe);
    const middot = 'Designer · Studio · 2020 – 2022';
    expect(roundTrip(middot)).toBe(middot);
  });

  it('inline bold in full-document context does not corrupt surrounding text', () => {
    const md = '## Summary\nEngineer with **10 years** of experience.\n\n## Skills\nRust, Go';
    expect(roundTrip(md)).toBe(md);
  });

  it('inline italic in full-document context does not corrupt surrounding text', () => {
    const md = 'Experience includes *distributed systems* and cloud platforms.';
    expect(roundTrip(md)).toBe(md);
  });

  it('inline link on a contact header line survives byte-exact', () => {
    const md =
      'Alex Kim\nalex@example.com | [LinkedIn](https://linkedin.com/in/alex) | [GitHub](https://github.com/alex)';
    expect(roundTrip(md)).toBe(md);
  });

  it('ALL-CAPS banner heading survives byte-exact (not re-marked as ## heading)', () => {
    const md = 'PROFESSIONAL EXPERIENCE\n\nSenior Engineer  Acme  2020 – Present';
    expect(roundTrip(md)).toBe(md);
  });

  it('known section name heading (Experience, Skills) survives byte-exact', () => {
    const md = 'Experience\n\nSenior Engineer  Acme  2020 – Present\n\nSkills\nRust, Go';
    expect(roundTrip(md)).toBe(md);
  });

  it('custom ## heading survives byte-exact', () => {
    const md = '## Side Projects\n- Built a CLI tool\n- Contributed to open source';
    expect(roundTrip(md)).toBe(md);
  });

  it('H3 subheading (###) survives byte-exact', () => {
    const md = '## Projects\n### Open Source\n- rust-analyzer\n- tokio';
    expect(roundTrip(md)).toBe(md);
  });

  it('flat bullet list survives byte-exact', () => {
    const md = '## Skills\n- TypeScript\n- Rust\n- Go\n- PostgreSQL';
    expect(roundTrip(md)).toBe(md);
  });

  it('blank lines between sections preserved exactly (not added or removed)', () => {
    const md = '## Summary\nGreat candidate.\n\n## Experience\nSenior Engineer\n\n## Skills\nRust';
    expect(roundTrip(md)).toBe(md);
  });

  it('name + contact header block survives byte-exact', () => {
    const md = 'Jane Doe\njane.doe@example.com | +31 6 12345678 | linkedin.com/in/janedoe';
    expect(roundTrip(md)).toBe(md);
  });
});

// ── 3. splitPreserved / joinPreserved direct unit assertions ─────────────────

describe('splitPreserved / joinPreserved: direct unit assertions', () => {
  const BODY =
    '## Summary\nGreat candidate.\n\n## Experience\nSenior Engineer  Acme  2020 – Present\n- Did the thing.';
  const LINK_BLOCK =
    '\n---\n- [LinkedIn](https://linkedin.com/in/x)\n- [GitHub](https://github.com/x)\n- [Portfolio](https://example.com)';

  it('tail is detected: splitPreserved returns the link block as tail', () => {
    const { tail } = splitPreserved(BODY + LINK_BLOCK);
    expect(tail).toBe(LINK_BLOCK);
  });

  it('body excludes the link block entirely', () => {
    const { body } = splitPreserved(BODY + LINK_BLOCK);
    expect(body).toBe(BODY);
    expect(body).not.toContain('---');
    expect(body).not.toContain('[LinkedIn]');
  });

  it('joinPreserved restores the original document byte-exact', () => {
    const full = BODY + LINK_BLOCK;
    const { body, tail } = splitPreserved(full);
    expect(joinPreserved(body, tail)).toBe(full);
  });

  it('empty tail: no link block → tail is empty string', () => {
    const { body, tail } = splitPreserved(BODY);
    expect(body).toBe(BODY);
    expect(tail).toBe('');
  });

  it('joinPreserved with empty tail returns body unchanged', () => {
    expect(joinPreserved(BODY, '')).toBe(BODY);
  });

  it('link block with a single entry is still detected', () => {
    const single = BODY + '\n---\n- [LinkedIn](https://linkedin.com/in/x)';
    const { body, tail } = splitPreserved(single);
    expect(body).toBe(BODY);
    expect(tail).toBe('\n---\n- [LinkedIn](https://linkedin.com/in/x)');
  });

  it('uses LAST ---  separator (body may contain an earlier --- that is not a link block)', () => {
    // A --- in the body that is followed by non-link lines should NOT be the tail.
    const withEarlyRule = 'Intro text\n---\nNot a link line\n\n' + BODY + LINK_BLOCK;
    const { body, tail } = splitPreserved(withEarlyRule);
    // The link block is at the end — that one should be detected.
    expect(tail).toBe(LINK_BLOCK);
    expect(body).toBe('Intro text\n---\nNot a link line\n\n' + BODY);
  });

  it('a --- block NOT followed exclusively by link lines is NOT treated as tail', () => {
    const noTail = BODY + '\n---\nSome prose that is not a link line';
    const { tail } = splitPreserved(noTail);
    expect(tail).toBe('');
  });

  it('roundTrip leaves the held-out link block verbatim — byte-exact', () => {
    const full = BODY + LINK_BLOCK;
    expect(roundTrip(full)).toBe(full);
  });

  it('tail including trailing newline survives round-trip byte-exact', () => {
    // Some generated docs end with a newline after the last link line.
    const withTrailingNL = BODY + LINK_BLOCK + '\n';
    // splitPreserved should still detect the block; the trailing newline is
    // an empty line which is allowed by the validator (length === 0 is ok).
    const result = roundTrip(withTrailingNL);
    expect(result).toBe(withTrailingNL);
  });
});

// ── 4. Targeted edit tests ───────────────────────────────────────────────────
//
// Strategy: parse a document → apply ONE programmatic ProseMirror transform →
// docToMarkdown → assert (a) intended change is present, (b) everything else
// — especially job-entry date lines and the link block — is unchanged.
//
// We use the ProseMirror Node API directly (no live editor needed) to keep
// these tests headless and deterministic.

describe('targeted edit tests: single edit, rest unchanged', () => {
  const schema = getEditorSchema();

  // Base document used for edit tests — contains all risk elements.
  const BASE_MD = [
    'Jordan Lee',
    'jordan@example.com | [LinkedIn](https://linkedin.com/in/jordan)',
    '',
    '## Experience',
    '',
    'Senior Engineer  Acme Corp  Jan 2020 – Present',
    '- Led the platform team.',
    '- Shipped 4 major features.',
    '',
    'Staff Engineer, Contoso (Mar 2018 – Dec 2019)',
    '- Rewrote billing system.',
    '',
    'Junior Engineer | Widget Co | 2016 – 2018',
    '- Built REST APIs.',
    '',
    '## Skills',
    '- Go, Rust, TypeScript',
  ].join('\n');
  const LINK_BLOCK =
    '\n---\n- [LinkedIn](https://linkedin.com/in/jordan)\n- [GitHub](https://github.com/jordan)';
  const FULL_MD = BASE_MD + LINK_BLOCK;

  it('bolding a word: intended bold is present; link block and dates unchanged', () => {
    // Parse → locate the "Led the platform team." paragraph (a list item) →
    // rebuild with bold mark on "Led" → serialize.
    //
    // Approach: build a replacement doc that adds **Led** at the start of
    // the first bullet item under ## Experience.
    const { body, tail } = splitPreserved(FULL_MD);

    // Re-build: replace "Led the platform team." bullet with "**Led** the platform team."
    const modifiedMd = body.replace('Led the platform team.', '**Led** the platform team.');
    const editedDoc = markdownToDoc(modifiedMd, schema);
    const result = joinPreserved(docToMarkdown(editedDoc), tail);

    // (a) The intended bold is present.
    expect(result).toContain('**Led** the platform team.');
    // (b) All three job-entry date forms are intact — byte-exact substrings.
    expect(result).toContain('Senior Engineer  Acme Corp  Jan 2020 – Present');
    expect(result).toContain('Staff Engineer, Contoso (Mar 2018 – Dec 2019)');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    // (c) Link block is verbatim.
    expect(result).toContain(LINK_BLOCK);
    // (d) Double-space did not collapse — byte-exact pin (2 spaces, no more, no less).
    expect(result).toContain('Senior Engineer  Acme Corp  Jan 2020 – Present');
  });

  it('adding a bullet item: new item present; dates and link block unchanged', () => {
    const { body, tail } = splitPreserved(FULL_MD);
    // Add a new bullet to the Skills section.
    const modifiedBody = body.replace(
      '## Skills\n- Go, Rust, TypeScript',
      '## Skills\n- Go, Rust, TypeScript\n- PostgreSQL, Redis'
    );
    const editedDoc = markdownToDoc(modifiedBody, schema);
    const result = joinPreserved(docToMarkdown(editedDoc), tail);

    // (a) New bullet is present.
    expect(result).toContain('- PostgreSQL, Redis');
    // (b) Existing bullets unchanged.
    expect(result).toContain('- Go, Rust, TypeScript');
    // (c) Job-entry date lines intact.
    expect(result).toContain('Senior Engineer  Acme Corp  Jan 2020 – Present');
    expect(result).toContain('Staff Engineer, Contoso (Mar 2018 – Dec 2019)');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    // (d) Link block verbatim.
    expect(result).toContain(LINK_BLOCK);
  });

  it('inserting a link in body: link present; double-space dates and link block unchanged', () => {
    const { body, tail } = splitPreserved(FULL_MD);
    const modifiedBody = body.replace(
      'Rewrote billing system.',
      'Rewrote billing system using [Go](https://go.dev).'
    );
    const editedDoc = markdownToDoc(modifiedBody, schema);
    const result = joinPreserved(docToMarkdown(editedDoc), tail);

    // (a) Link is present.
    expect(result).toContain('[Go](https://go.dev)');
    // (b) Bullet text otherwise intact.
    expect(result).toContain('Rewrote billing system using [Go](https://go.dev).');
    // (c) All date forms intact.
    expect(result).toContain('Senior Engineer  Acme Corp  Jan 2020 – Present');
    expect(result).toContain('Staff Engineer, Contoso (Mar 2018 – Dec 2019)');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    // (d) Link block not touched.
    expect(result).toContain(LINK_BLOCK);
  });

  it('adding a ## heading: heading present; dates and link block unchanged', () => {
    const { body, tail } = splitPreserved(FULL_MD);
    const modifiedBody = body + '\n\n## Certifications\nAWS Solutions Architect (2023)';
    const editedDoc = markdownToDoc(modifiedBody, schema);
    const result = joinPreserved(docToMarkdown(editedDoc), tail);

    // (a) New heading present.
    expect(result).toContain('## Certifications');
    expect(result).toContain('AWS Solutions Architect (2023)');
    // (b) Date forms intact.
    expect(result).toContain('Senior Engineer  Acme Corp  Jan 2020 – Present');
    expect(result).toContain('Staff Engineer, Contoso (Mar 2018 – Dec 2019)');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    // (c) Link block untouched.
    expect(result).toContain(LINK_BLOCK);
  });

  it('edit does not corrupt the contact header line', () => {
    const { body, tail } = splitPreserved(FULL_MD);
    // Simulate editing the summary (adding a sentence after it).
    const modifiedBody = body.replace(
      '## Experience',
      '## Summary\nEngineered systems at scale.\n\n## Experience'
    );
    const editedDoc = markdownToDoc(modifiedBody, schema);
    const result = joinPreserved(docToMarkdown(editedDoc), tail);

    // Header line intact.
    expect(result).toContain('jordan@example.com | [LinkedIn](https://linkedin.com/in/jordan)');
    // Link block intact.
    expect(result).toContain(LINK_BLOCK);
  });

  it('italic applied: italic present; surrounding job-entry lines byte-exact', () => {
    const { body, tail } = splitPreserved(FULL_MD);
    const modifiedBody = body.replace('Built REST APIs.', 'Built *REST* APIs.');
    const editedDoc = markdownToDoc(modifiedBody, schema);
    const result = joinPreserved(docToMarkdown(editedDoc), tail);

    expect(result).toContain('Built *REST* APIs.');
    expect(result).toContain('Junior Engineer | Widget Co | 2016 – 2018');
    expect(result).toContain(LINK_BLOCK);
  });

  // Verify that the doc produced by markdownToDoc is round-trippable via
  // docToMarkdown independently from the full roundTrip helper.
  it('markdownToDoc → docToMarkdown round-trips the base document independently', () => {
    const { body, tail } = splitPreserved(FULL_MD);
    const doc = markdownToDoc(body, schema);
    expect(joinPreserved(docToMarkdown(doc), tail)).toBe(FULL_MD);
  });
});

// ── 5. Edge / negative cases ─────────────────────────────────────────────────

describe('edge and negative cases', () => {
  it('empty string: roundTrip returns empty string (no content)', () => {
    // Empty body → single empty paragraph → serializes as '' (one empty line = '').
    // This must not throw.
    expect(() => roundTrip('')).not.toThrow();
    // The serializer emits a single empty paragraph for an empty doc.
    // An empty doc produces a doc with one empty paragraph, which serializes to ''.
    expect(roundTrip('')).toBe('');
  });

  it('doc with NO link block: tail is empty string, body is the whole doc', () => {
    const md = '## Summary\nGreat candidate.\n\n## Skills\n- Rust\n- Go';
    const { body, tail } = splitPreserved(md);
    expect(tail).toBe('');
    expect(body).toBe(md);
    expect(roundTrip(md)).toBe(md);
  });

  it('literal · (middot) in body: NOT markdown-escaped on serialize', () => {
    const md = 'Role · Company · 2020 – Present';
    expect(roundTrip(md)).toBe(md);
    // Explicitly confirm no escaping artifact.
    expect(roundTrip(md)).not.toContain('\\·');
  });

  it('literal | (pipe) in body: NOT markdown-escaped on serialize', () => {
    const md = 'Role | Company | 2016 – 2018';
    expect(roundTrip(md)).toBe(md);
    expect(roundTrip(md)).not.toContain('\\|');
  });

  it('literal ( and ) in body: NOT markdown-escaped on serialize', () => {
    const md = 'Staff Engineer, Contoso (Mar 2018 – Dec 2019)';
    expect(roundTrip(md)).toBe(md);
    expect(roundTrip(md)).not.toContain('\\(');
    expect(roundTrip(md)).not.toContain('\\)');
  });

  it('literal - at start of paragraph (not a bullet): NOT treated as bullet', () => {
    // A line starting with "- " IS a bullet. A plain hyphen mid-sentence is not.
    const md = 'AWS Solutions Architect — Associate (2022)';
    expect(roundTrip(md)).toBe(md);
  });

  it('consecutive blank lines: preserved as distinct block separators', () => {
    // Two consecutive blank lines → two empty paragraphs → two blank lines.
    const md = '## Section A\nContent.\n\n\n## Section B\nMore content.';
    expect(roundTrip(md)).toBe(md);
  });

  it('@ symbol in email: NOT escaped', () => {
    const md = 'jane.doe@example.com | +31 6 12345678';
    expect(roundTrip(md)).toBe(md);
    expect(roundTrip(md)).not.toContain('\\@');
  });

  it('em-dash (–) in date range: preserved verbatim', () => {
    const md = 'Senior Engineer  Acme Corp  Jan 2020 – Present';
    expect(roundTrip(md)).toBe(md);
    // Must be the en-dash U+2013, not a hyphen.
    expect(roundTrip(md)).toContain('–');
  });

  it('link inside a bullet item: survives round-trip byte-exact', () => {
    const md =
      '## Projects\n- [rust-http-client](https://github.com/alexkim/rust-http): async Rust HTTP client.';
    expect(roundTrip(md)).toBe(md);
  });

  it('link URL with a balanced (...) inside it survives byte-exact (no truncation)', () => {
    // Wiki-style URL containing a nested, balanced paren. The inline-link URL
    // pattern allows one level of nesting, so the URL is captured to its FINAL
    // ) rather than truncated at the first interior ).
    const md = '[C](https://en.wikipedia.org/wiki/C_(programming_language))';
    expect(roundTrip(md)).toBe(md);
  });

  it('link with nested-paren URL followed by literal text does not over-consume', () => {
    // Ensure the balanced-paren URL does not greedily swallow a trailing ) that
    // belongs to surrounding prose.
    const md = 'See [C](https://en.wikipedia.org/wiki/C_(programming_language)) (the language).';
    expect(roundTrip(md)).toBe(md);
  });

  it('plain link followed by literal text containing ) is unaffected', () => {
    const md = 'Built [Workers KV](https://developers.cloudflare.com/kv/) (fast).';
    expect(roundTrip(md)).toBe(md);
  });

  it('bold inside a heading: survives round-trip byte-exact', () => {
    // This is unusual but valid per the locked schema.
    const md = '## Summary\n**Lead** engineer with 10 years of experience.';
    expect(roundTrip(md)).toBe(md);
  });

  it('italic inside a bullet: survives round-trip byte-exact', () => {
    const md = '## Skills\n- *TypeScript*, Rust, Go';
    expect(roundTrip(md)).toBe(md);
  });

  it('multiple links on one line (contact header): all survive byte-exact', () => {
    const md =
      'Alex Kim\nalex@example.com | [LinkedIn](https://linkedin.com/in/alexkim) | [GitHub](https://github.com/alexkim) | [Portfolio](https://alexkim.dev)';
    expect(roundTrip(md)).toBe(md);
  });

  it('splitPreserved: link block with empty trailing line still detected', () => {
    const body = '## Summary\nContent.';
    const tail = '\n---\n- [LinkedIn](https://linkedin.com/in/x)\n';
    const full = body + tail;
    const split = splitPreserved(full);
    expect(split.body).toBe(body);
    expect(split.tail).toBe(tail);
  });

  it('getEditorSchema: returns a ProseMirror Schema with required node types', () => {
    const s = getEditorSchema();
    // The locked schema must have these nodes for the round-trip to work.
    expect(s.nodes['doc']).toBeDefined();
    expect(s.nodes['paragraph']).toBeDefined();
    expect(s.nodes['heading']).toBeDefined();
    expect(s.nodes['bulletList']).toBeDefined();
    expect(s.nodes['listItem']).toBeDefined();
    expect(s.marks['bold']).toBeDefined();
    expect(s.marks['italic']).toBeDefined();
    expect(s.marks['link']).toBeDefined();
    // Disabled nodes must NOT be in the schema (locked schema guarantee).
    expect(s.nodes['codeBlock']).toBeUndefined();
    expect(s.nodes['blockquote']).toBeUndefined();
    expect(s.nodes['orderedList']).toBeUndefined();
  });

  it('real fixture (resume.txt) round-trips via markdownToDoc → docToMarkdown independently', () => {
    // Test the individual functions, not just the high-level roundTrip helper.
    const { body, tail } = splitPreserved(CORPUS_REAL_RESUME);
    expect(tail).toBe(''); // real fixture has no link block — confirm assumption.
    const doc = markdownToDoc(body);
    const serialized = docToMarkdown(doc);
    expect(serialized).toBe(body);
  });
});
