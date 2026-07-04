import { describe, expect, it } from 'vitest';

import urlLabels from '../fixtures/url-labels.json';
import {
  APPLICATION_QUESTIONS,
  buildApplicantDetailsBlock,
  buildApplicationAnswerPrompt,
  buildApplicationAnswerSystemPrompt,
  buildBodyLinksBlock,
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildEmphasisDirectivesBlock,
  buildGroundingBlock,
  buildMetadataPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  EMPHASIS_OPTIONS,
  type EmphasisId,
  extractPlainText,
  type GenerationMeta,
  getBodyLinkMap,
  getLinkMap,
  injectLinksIntoGeneratedText,
  MODES,
  parseLinksFromResume,
  resumeMentions,
  urlToFriendlyLabel,
  validateMetadata,
} from './index';

const RESUME_WITH_LINKS = `John Doe
Senior Engineer
Berlin, Germany | john@example.com

PROFESSIONAL SUMMARY
Built lots of things.

---
- [LinkedIn](https://linkedin.com/in/johndoe)
- [GitHub](https://github.com/johndoe)
- [Email](mailto:john@example.com)
- [Personal](https://not-a-profile.example.com)`;

const META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'John Doe',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  targetLanguage: 'en',
  topRequirements: ['React', 'TypeScript', 'AWS'],
};

describe('MODES', () => {
  it('defines tone instructions for every mode', () => {
    for (const mode of Object.keys(MODES) as (keyof typeof MODES)[]) {
      expect(MODES[mode].label).toBeTruthy();
      expect(MODES[mode].toneInstruction.length).toBeGreaterThan(10);
    }
  });
});

describe('getLinkMap', () => {
  it('maps profile labels to URLs, drops email, and admits one Website link', () => {
    const map = getLinkMap(RESUME_WITH_LINKS);
    expect(map.LinkedIn).toBe('https://linkedin.com/in/johndoe');
    expect(map.GitHub).toBe('https://github.com/johndoe');
    expect(map.Email).toBeUndefined();
    // The single non-platform URL is admitted under a generic "Website" label —
    // never under its raw anchor text.
    expect(map.Website).toBe('https://not-a-profile.example.com');
    expect(map.Personal).toBeUndefined();
  });

  it('admits exactly one Website link; later non-platform URLs become body links (#18)', () => {
    const resume = [
      'Body',
      '---',
      '- [LinkedIn](https://linkedin.com/in/jane)',
      '- [Portfolio](https://janedoe.dev)',
      '- [Blog](https://janeblog.example)',
      '- [Email](mailto:jane@example.com)',
    ].join('\n');
    const map = getLinkMap(resume);
    expect(map.LinkedIn).toBe('https://linkedin.com/in/jane');
    expect(map.Website).toBe('https://janedoe.dev'); // first bare-root non-platform wins
    expect(Object.values(map)).not.toContain('https://janeblog.example'); // 2nd → body, not contact
    expect(Object.values(map)).not.toContain('mailto:jane@example.com'); // mailto dropped
    // The second personal site is no longer dropped — it is preserved as a body link.
    expect(getBodyLinkMap(resume).Blog).toBe('https://janeblog.example');
  });

  it('returns an empty map when there is no reference block', () => {
    expect(getLinkMap('Just a plain resume with no separator')).toEqual({});
  });

  it('derives a friendly label when the anchor is a raw URL', () => {
    const resume = `Body\n---\n- [https://github.com/jane](https://github.com/jane)`;
    const map = getLinkMap(resume);
    expect(map.GitHub).toBe('https://github.com/jane');
  });

  it('keeps a GitHub profile (one path segment) on the contact line', () => {
    const map = getLinkMap('Body\n---\n- [GitHub](https://github.com/jane)');
    expect(map.GitHub).toBe('https://github.com/jane');
    expect(getBodyLinkMap('Body\n---\n- [GitHub](https://github.com/jane)')).toEqual({});
  });
});

describe('getBodyLinkMap (#18 — body links)', () => {
  it('classifies project / publication / repo links as body, not contact', () => {
    const resume = [
      'Body',
      '---',
      '- [LinkedIn](https://linkedin.com/in/jane)', // contact profile
      '- [GitHub](https://github.com/jane)', // contact profile (1 segment)
      '- [orbit-sim](https://github.com/jane/orbit-sim)', // deep repo → body
      '- [Spin glasses in 2D](https://doi.org/10.1103/PhysRevB.1.234)', // publication → body
      '- [Email](mailto:jane@example.com)',
    ].join('\n');

    const contact = getLinkMap(resume);
    expect(contact.LinkedIn).toBe('https://linkedin.com/in/jane');
    expect(contact.GitHub).toBe('https://github.com/jane');

    const body = getBodyLinkMap(resume);
    expect(body['orbit-sim']).toBe('https://github.com/jane/orbit-sim');
    expect(body['Spin glasses in 2D']).toBe('https://doi.org/10.1103/PhysRevB.1.234');
    // The repo did NOT pollute the contact map, and the profile is not a body link.
    expect(contact['orbit-sim']).toBeUndefined();
    expect(body.GitHub).toBeUndefined();
  });

  it('humanises a slug when a body link anchor is a raw URL (PDF case)', () => {
    const resume =
      'Body\n---\n- [https://example.org/my-research-paper](https://example.org/my-research-paper)';
    expect(getBodyLinkMap(resume)['my research paper']).toBe(
      'https://example.org/my-research-paper'
    );
  });

  it('returns an empty map when there are no body links', () => {
    expect(getBodyLinkMap(RESUME_WITH_LINKS)).toEqual({});
  });
});

describe('injectLinksIntoGeneratedText', () => {
  it('replaces known labels in the contact line with markdown links', () => {
    const text = `John Doe\nSenior Engineer\nBerlin | john@example.com | LinkedIn | GitHub\n\nSUMMARY`;
    const out = injectLinksIntoGeneratedText(text, {
      LinkedIn: 'https://linkedin.com/in/jd',
      GitHub: 'https://github.com/jd',
    });
    expect(out).toContain('[LinkedIn](https://linkedin.com/in/jd)');
    expect(out).toContain('[GitHub](https://github.com/jd)');
  });

  it('returns text unchanged when the link map is empty', () => {
    const text = 'Name\nRole\nCity | LinkedIn';
    expect(injectLinksIntoGeneratedText(text, {})).toBe(text);
  });

  it('does not touch section header lines', () => {
    const text = `WORK EXPERIENCE | something\nbody`;
    const out = injectLinksIntoGeneratedText(text, { LinkedIn: 'https://linkedin.com/in/x' });
    expect(out).toBe(text);
  });

  it('injects a Website link in the contact line', () => {
    const text = `Jane Doe\nDesigner\nBerlin | jane@example.com | Website | GitHub\n\nSUMMARY`;
    const out = injectLinksIntoGeneratedText(text, {
      Website: 'https://janedoe.dev',
      GitHub: 'https://github.com/jd',
    });
    expect(out).toContain('[Website](https://janedoe.dev)');
    expect(out).toContain('[GitHub](https://github.com/jd)');
  });

  it('finds the cover-letter contact line below the top (past the old 6-line window)', () => {
    // Regression: cover letters carry the contact line under a marker / name /
    // preamble, so the old fixed first-6-lines scan silently skipped it and
    // LinkedIn never got hyperlinked (Dribbble survived only as a bare URL).
    const coverLetter = [
      'COMPLETE COVER LETTER ###',
      '',
      'preamble one',
      'preamble two',
      'preamble three',
      'preamble four',
      'preamble five',
      'Lena Vos',
      'Amsterdam, Niederlande | lena.vos@example.com | +31 6 | LinkedIn | Dribbble',
      '',
      'Sehr geehrte Damen und Herren,',
    ].join('\n');
    const out = injectLinksIntoGeneratedText(coverLetter, {
      LinkedIn: 'https://linkedin.com/in/lena-vos',
      Dribbble: 'https://dribbble.com/lenavos',
    });
    expect(out).toContain('[LinkedIn](https://linkedin.com/in/lena-vos)');
    expect(out).toContain('[Dribbble](https://dribbble.com/lenavos)');
  });

  it('links only the email-bearing contact line, not body prose mentioning a platform', () => {
    const text = [
      'Lena Vos',
      'Amsterdam | lena.vos@example.com | LinkedIn',
      '',
      'I doubled our GitHub | community and shipped on LinkedIn weekly.',
    ].join('\n');
    const out = injectLinksIntoGeneratedText(text, {
      LinkedIn: 'https://linkedin.com/in/lena-vos',
      GitHub: 'https://github.com/lenavos',
    });
    expect(out).toContain('[LinkedIn](https://linkedin.com/in/lena-vos)');
    // The body sentence has a pipe but no email → left untouched.
    expect(out).toContain('I doubled our GitHub | community and shipped on LinkedIn weekly.');
    expect(out).not.toContain('[GitHub](https://github.com/lenavos)');
  });

  it('is idempotent — a second pass does not double-wrap links', () => {
    const text = 'Name\nCity | n@example.com | LinkedIn';
    const once = injectLinksIntoGeneratedText(text, { LinkedIn: 'https://linkedin.com/in/n' });
    const twice = injectLinksIntoGeneratedText(once, { LinkedIn: 'https://linkedin.com/in/n' });
    expect(twice).toBe(once);
  });

  it('injects body links onto their items anywhere in the body, not just the contact line (#18)', () => {
    const text = [
      'Jane Dev',
      'Researcher',
      'Berlin | jane@example.com | GitHub',
      '',
      'PROJECTS',
      '• orbit-sim — a relativistic orbit simulator',
      '',
      'PUBLICATIONS',
      '• Spin glasses in 2D, Phys Rev B (2021)',
    ].join('\n');
    const out = injectLinksIntoGeneratedText(
      text,
      { GitHub: 'https://github.com/janedev' },
      {
        'orbit-sim': 'https://github.com/janedev/orbit-sim',
        'Spin glasses in 2D': 'https://doi.org/10.1/x',
      }
    );
    expect(out).toContain('[GitHub](https://github.com/janedev)'); // contact line
    expect(out).toContain('[orbit-sim](https://github.com/janedev/orbit-sim)'); // project bullet
    expect(out).toContain('[Spin glasses in 2D](https://doi.org/10.1/x)'); // publication bullet
  });

  it('body-link injection is idempotent and skips already-linked spans', () => {
    const text = '• orbit-sim — a simulator';
    const map = { 'orbit-sim': 'https://github.com/janedev/orbit-sim' };
    const once = injectLinksIntoGeneratedText(text, {}, map);
    const twice = injectLinksIntoGeneratedText(once, {}, map);
    expect(once).toContain('[orbit-sim](https://github.com/janedev/orbit-sim)');
    expect(twice).toBe(once);
  });

  it('does not inject body links when no bodyMap is passed (cover-letter path)', () => {
    const text = '• orbit-sim — a simulator';
    expect(injectLinksIntoGeneratedText(text, { GitHub: 'https://github.com/x' })).toBe(text);
  });
});

describe('parseLinksFromResume', () => {
  it('extracts a clean email, profile labels, and the Website label', () => {
    const { block, cleanEmail } = parseLinksFromResume(RESUME_WITH_LINKS);
    expect(cleanEmail).toBe('john@example.com');
    expect(block).toContain('LinkedIn');
    expect(block).toContain('GitHub');
    expect(block).toContain('Website'); // non-platform URL surfaced for the AI to write
  });

  it('returns empty result when there is no reference block', () => {
    expect(parseLinksFromResume('No block here')).toEqual({ block: '', cleanEmail: '' });
  });
});

describe('buildBodyLinksBlock (#18)', () => {
  it('lists body link labels and instructs the model to keep them on their items', () => {
    const resume = [
      'Body',
      '---',
      '- [LinkedIn](https://linkedin.com/in/jane)',
      '- [orbit-sim](https://github.com/jane/orbit-sim)',
      '- [My thesis](https://doi.org/10.1/x)',
    ].join('\n');
    const block = buildBodyLinksBlock(resume);
    expect(block).toContain('orbit-sim');
    expect(block).toContain('My thesis');
    expect(block).toContain('PROJECTS');
    // Contact-line links must NOT appear in the body block.
    expect(block).not.toContain('LinkedIn');
  });

  it('returns an empty string when there are no body links', () => {
    expect(buildBodyLinksBlock(RESUME_WITH_LINKS)).toBe('');
  });
});

describe('urlToFriendlyLabel ↔ Rust url_label parity', () => {
  // Shared source of truth with `cargo test export::links` — both suites read
  // fixtures/url-labels.json so the two implementations can never silently drift.
  it('matches the shared fixture for every URL', () => {
    const cases = urlLabels as { url: string; label: string }[];
    expect(cases.length).toBeGreaterThan(0);
    for (const { url, label } of cases) {
      expect(urlToFriendlyLabel(url)).toBe(label);
    }
  });
});

describe('buildMetadataPrompt', () => {
  it('produces a JSON-extraction prompt for large models', () => {
    const { system, user } = buildMetadataPrompt(RESUME_WITH_LINKS, 'Job ad text');
    expect(system).toContain('document parser');
    expect(user).toContain('<candidate_resume>');
    expect(user).toContain('<job_ad>');
    expect(user).not.toContain('Example output:'); // one-shot is small-model only
  });

  it('adds a one-shot example for small models', () => {
    const { user } = buildMetadataPrompt(RESUME_WITH_LINKS, 'Job ad', 'small');
    expect(user).toContain('Example output:');
  });
});

describe('buildResumeSystemPrompt', () => {
  it('returns a detailed prompt for large models', () => {
    const prompt = buildResumeSystemPrompt('ats');
    expect(prompt).toContain('ATS OPTIMIZATION RULES');
    expect(prompt).toContain(MODES.ats.label);
  });

  it('returns a compact prompt for small models', () => {
    const prompt = buildResumeSystemPrompt('technical', 'small');
    expect(prompt).toContain('NEVER BREAK THESE RULES');
    expect(prompt.length).toBeLessThan(buildResumeSystemPrompt('technical').length);
  });

  it('forbids dropping work roles in every depth', () => {
    expect(buildResumeSystemPrompt('ats')).toMatch(/NEVER drop, merge, or omit a work role/i);
    expect(buildResumeSystemPrompt('ats', 'small')).toMatch(/NEVER omit a work role/i);
  });
});

describe('buildEmphasisDirectivesBlock (#15)', () => {
  it('returns empty for no/empty selection', () => {
    expect(buildEmphasisDirectivesBlock(undefined)).toBe('');
    expect(buildEmphasisDirectivesBlock([])).toBe('');
  });

  it('emits one instruction per selected directive, in registry order, with a no-fabrication guard', () => {
    const block = buildEmphasisDirectivesBlock(['technical', 'quantify']);
    expect(block).toContain('WITHOUT inventing facts');
    // Registry order (quantify before technical) regardless of input order.
    expect(block.indexOf('Quantify impact')).toBeLessThan(block.indexOf('Technical depth'));
    // Exactly two directive lines.
    expect(block.split('\n').filter((l) => l.startsWith('- ')).length).toBe(2);
  });

  it('ignores unknown ids and de-dupes repeats', () => {
    // Cast simulates a stale/unknown id leaking from persisted state.
    const ids = ['quantify', 'quantify', 'bogus'] as EmphasisId[];
    const block = buildEmphasisDirectivesBlock(ids);
    expect(block.split('\n').filter((l) => l.startsWith('- ')).length).toBe(1);
  });

  it('every registry option carries a fact-safe instruction', () => {
    expect(EMPHASIS_OPTIONS.length).toBeGreaterThanOrEqual(5);
    for (const o of EMPHASIS_OPTIONS) {
      expect(o.instruction.length).toBeGreaterThan(20);
    }
  });
});

describe('buildResumePrompt', () => {
  it('includes candidate context and a language note', () => {
    const prompt = buildResumePrompt(RESUME_WITH_LINKS, 'Job ad', META, 'ats');
    expect(prompt).toContain('John Doe');
    expect(prompt).toContain('Write in en.');
    expect(prompt).toContain('**React**');
  });

  it('emits a translation note when languages mismatch', () => {
    const prompt = buildResumePrompt(
      RESUME_WITH_LINKS,
      'Job ad',
      { ...META, mismatch: true },
      'ats'
    );
    expect(prompt).toContain('Rewrite entirely');
  });

  it('keeps every role and drops the old culling instructions', () => {
    const prompt = buildResumePrompt(RESUME_WITH_LINKS, 'Job ad', META, 'ats');
    expect(prompt).toContain('Include EVERY role');
    expect(prompt).toContain('Repeat the block above for EVERY role');
    // The instructions that told the model to cull roles must be gone.
    expect(prompt).not.toContain('remove bullets irrelevant');
    expect(prompt).not.toContain('experience to minimize');
    expect(prompt).not.toContain('experience items most relevant');
  });

  it('folds in emphasis directives only when selected (#15)', () => {
    const base = buildResumePrompt(RESUME_WITH_LINKS, 'Job ad', META, 'ats');
    expect(base).not.toContain('EMPHASIS — apply these user-selected biases');

    const withEmphasis = buildResumePrompt(
      RESUME_WITH_LINKS,
      'Job ad',
      { ...META, emphasis: ['quantify', 'concise'] },
      'ats'
    );
    expect(withEmphasis).toContain('EMPHASIS — apply these user-selected biases');
    expect(withEmphasis).toContain('Quantify impact');
    expect(withEmphasis).toContain('More concise');
  });

  it('surfaces body project/publication links so they survive generation (#18)', () => {
    const resume = [
      'Jane Dev',
      'Researcher',
      'Berlin | jane@example.com',
      '',
      'PROJECTS',
      'Built orbit-sim',
      '',
      '---',
      '- [orbit-sim](https://github.com/jane/orbit-sim)',
      '- [My thesis](https://doi.org/10.1/x)',
    ].join('\n');
    const prompt = buildResumePrompt(resume, 'Job ad', META, 'ats');
    expect(prompt).toContain('CANDIDATE PROJECT / PUBLICATION LINKS');
    expect(prompt).toContain('orbit-sim');
    expect(prompt).toContain('My thesis');
    // The raw reference block itself is still stripped from <candidate_resume>.
    expect(prompt).not.toContain('](https://doi.org/10.1/x)');
  });
});

const RESUME_FOR_GROUNDING = `Jane Dev
Senior Engineer
jane@example.com

PROFESSIONAL SUMMARY
Backend engineer who ships React apps written in TypeScript.

WORK EXPERIENCE
Acme — Engineer (2020 - Present)
Built services in TypeScript and React with PostgreSQL.

SKILLS
React, TypeScript, PostgreSQL`;

describe('resumeMentions', () => {
  it('matches single tokens on word boundaries (not substrings)', () => {
    expect(resumeMentions('Built React apps', 'React')).toBe(true);
    expect(resumeMentions('Worked in the category team', 'Go')).toBe(false);
    expect(resumeMentions('Wrote services in Go', 'Go')).toBe(true);
  });

  it('matches punctuated / multi-word terms as substrings', () => {
    expect(resumeMentions('Built with Node.js', 'node.js')).toBe(true);
    expect(resumeMentions('Designed a REST API for payments', 'REST API')).toBe(true);
    expect(resumeMentions('No cloud here', 'AWS')).toBe(false);
  });

  it('synonym path: JS alias matches JavaScript requirement', () => {
    // Résumé says "JS bundles"; requirement spells out "JavaScript".
    // The SYNONYMS map normalizes "js" → "javascript" on both sides.
    expect(resumeMentions('Shipped JS bundles and optimized load times', 'JavaScript')).toBe(true);
  });

  it('synonym path: k8s alias matches Kubernetes requirement', () => {
    // Résumé says "k8s clusters"; requirement spells out "Kubernetes".
    expect(resumeMentions('Ran k8s clusters on bare metal', 'Kubernetes')).toBe(true);
  });

  it('negative: java must NOT match javascript (word-boundary, no false alias)', () => {
    // "java" and "javascript" are different tokens; no synonym maps one to the other.
    expect(resumeMentions('Maintained Java microservices', 'javascript')).toBe(false);
  });

  it('punctuation edge: trailing comma on résumé token does not block alias match', () => {
    // "JavaScript," (trailing comma) must still match requirement "JavaScript".
    expect(
      resumeMentions('Shipped JavaScript, bundles and optimized load times', 'JavaScript')
    ).toBe(true);
  });

  it('punctuation edge: leading/trailing parens on résumé token do not block alias match', () => {
    // "(Kubernetes)" must still match requirement "Kubernetes".
    expect(resumeMentions('(Kubernetes) clusters on bare metal', 'Kubernetes')).toBe(true);
  });

  it('boundary trim: strips leading/trailing boundary punct, preserves internal punct', () => {
    // Trailing comma stripped → matches
    expect(resumeMentions('JavaScript, bundles shipped', 'JavaScript')).toBe(true);
    // Parens stripped → matches
    expect(resumeMentions('(Kubernetes) on-prem', 'Kubernetes')).toBe(true);
    // Internal punct preserved — c++ must not collapse to c
    expect(resumeMentions('shipped in c++', 'c++')).toBe(true);
    // Internal dot preserved — node.js must not collapse to node
    expect(resumeMentions('runs on node.js', 'node.js')).toBe(true);
  });

  it('redos regression: pathological punctuation token completes instantly (linear scan)', () => {
    // 100 000 consecutive quote chars — the old /^[...]+|[...]+$/g regex
    // backtracks polynomially on this input; the linear scan returns immediately.
    const pathological = '"'.repeat(100_000);
    const result = resumeMentions(pathological, 'JavaScript');
    // The entire token is boundary punctuation → stripped to '' → no match.
    expect(result).toBe(false);
  });
});

describe('buildGroundingBlock', () => {
  it('splits requirements into résumé-backed present vs absent', () => {
    const block = buildGroundingBlock(RESUME_FOR_GROUNDING, [
      'React',
      'TypeScript',
      'AWS',
      'Kubernetes',
    ]);
    expect(block).toContain('PRESENT');
    expect(block).toContain('React');
    expect(block).toContain('TypeScript');
    expect(block).toContain('ABSENT');
    expect(block).toContain('AWS');
    expect(block).toContain('Kubernetes');
  });

  it('returns empty string when there are no requirements', () => {
    expect(buildGroundingBlock(RESUME_FOR_GROUNDING, [])).toBe('');
  });
});

describe('résumé context wiring', () => {
  it('embeds the grounding split in the résumé prompt', () => {
    const prompt = buildResumePrompt(RESUME_FOR_GROUNDING, 'Job ad', META, 'ats');
    expect(prompt).toContain('SKILL GROUNDING');
    expect(prompt).toContain('PRESENT');
  });

  it('embeds the grounding split in the cover-letter prompt', () => {
    const prompt = buildCoverLetterPrompt(RESUME_FOR_GROUNDING, 'Job ad', META, 'recruiter');
    expect(prompt).toContain('SKILL GROUNDING');
  });

  it('no longer hard-cuts the résumé tail at 2500 chars for local tiers', () => {
    const tail = 'UNIQUE_TAIL_MARKER';
    const longResume = [
      'Jane Dev',
      'Senior Engineer',
      'jane@example.com',
      '',
      'PROFESSIONAL SUMMARY',
      'Experienced engineer. '.repeat(150), // ~3.3k chars, well past the old 2500 cap
      '',
      'SKILLS',
      `${tail} React, TypeScript`,
    ].join('\n');
    // 'medium' resolves to the brief depth that previously sliced at 2500 chars;
    // the résumé fits the section-aware token budget, so the tail survives.
    const prompt = buildResumePrompt(longResume, 'Job ad', META, 'ats', 'medium');
    expect(prompt).toContain(tail);
  });
});

describe('buildCoverLetterSystemPrompt', () => {
  it('returns a detailed prompt for large models', () => {
    const prompt = buildCoverLetterSystemPrompt('recruiter');
    // The detailed prompt teaches flow/voice (the fix for robotic output) via a
    // movement-by-movement narrative + a tone exemplar, and is materially longer
    // than the compact small-model variant.
    expect(prompt).toContain('cover letter specialist');
    expect(prompt).toContain('MOVEMENT BY MOVEMENT');
    expect(prompt).toContain('TONE REFERENCE');
    expect(prompt.length).toBeGreaterThan(
      buildCoverLetterSystemPrompt('recruiter', 'small').length
    );
  });

  it('carries the anti-bluff honesty spine in every depth', () => {
    // Matching the job ad must never become claiming résumé-absent skills, so the
    // no-bluff directive appears in the large (cloud), small (local), and agent
    // (cli/task) prompt variants.
    expect(buildCoverLetterSystemPrompt('ats', 'large')).toMatch(/never bluff/i);
    expect(buildCoverLetterSystemPrompt('ats', 'small')).toMatch(/never bluff/i);
    expect(buildCoverLetterSystemPrompt('ats', { kind: 'cli' })).toMatch(/never bluff/i);
  });

  it('returns a compact prompt for small models', () => {
    const prompt = buildCoverLetterSystemPrompt('recruiter', 'small');
    expect(prompt).toContain('cover letter writer');
  });
});

describe('buildCoverLetterPrompt', () => {
  it("includes today's date and the role context", () => {
    const prompt = buildCoverLetterPrompt(RESUME_WITH_LINKS, 'Job ad', META, 'recruiter');
    expect(prompt).toContain('Acme');
    expect(prompt).toContain('Today:');
  });

  it('omits the company-research block when no brief is provided', () => {
    const prompt = buildCoverLetterPrompt(RESUME_WITH_LINKS, 'Job ad', META, 'recruiter');
    expect(prompt).not.toContain('<company_research>');
  });

  it('folds in emphasis directives when selected (#15)', () => {
    const prompt = buildCoverLetterPrompt(
      RESUME_WITH_LINKS,
      'Job ad',
      { ...META, emphasis: ['leadership'] },
      'recruiter'
    );
    expect(prompt).toContain('EMPHASIS — apply these user-selected biases');
    expect(prompt).toContain('Leadership focus');
  });

  it('injects German market conventions (Betreff + salary/start-date) while keeping the letter language', () => {
    const prompt = buildCoverLetterPrompt(
      RESUME_WITH_LINKS,
      'Job ad',
      META,
      'recruiter',
      'large',
      '',
      'de'
    );
    expect(prompt).toContain('<market_conventions market="Germany">');
    expect(prompt).toContain('Betreff');
    expect(prompt).toMatch(/salary expectation/i);
    // Decision: write in the letter language (en here), apply German etiquette.
    expect(prompt).toMatch(/Write the letter in en/);
  });

  it('uses the international baseline (no subject line) by default', () => {
    const prompt = buildCoverLetterPrompt(RESUME_WITH_LINKS, 'Job ad', META, 'recruiter');
    expect(prompt).toContain('<market_conventions market="International">');
    expect(prompt).toContain('Do NOT add a subject line');
  });

  it('folds a provided company brief into a fenced, untrusted research block', () => {
    const brief = 'Acme builds payment rails for SMBs and recently raised a Series B.';
    const prompt = buildCoverLetterPrompt(
      RESUME_WITH_LINKS,
      'Job ad',
      META,
      'recruiter',
      'large',
      brief
    );
    expect(prompt).toContain('<company_research>');
    expect(prompt).toContain(brief);
    // Prompt-injection hardening: the brief is reference-only, and embedded
    // instructions must be ignored.
    expect(prompt).toMatch(/untrusted/i);
    expect(prompt).toMatch(/ignore any instructions/i);
    // Positive use: the prompt now tells the model to actually weave the brief
    // into the "why this company" part, so research informs the letter instead
    // of just being fenced and ignored.
    expect(prompt).toMatch(/draw on <company_research>/i);
    expect(prompt).toMatch(/why this company/i);
  });
});

describe('application questions', () => {
  it('exposes a non-empty registry with unique ids', () => {
    expect(APPLICATION_QUESTIONS.length).toBeGreaterThan(0);
    const ids = APPLICATION_QUESTIONS.map((q) => q.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const q of APPLICATION_QUESTIONS) expect(q.question.length).toBeGreaterThan(5);
  });

  it('system prompt enforces no-fabrication grounding', () => {
    const sys = buildApplicationAnswerSystemPrompt();
    expect(sys).toMatch(/traceable to <candidate_resume>/i);
    expect(sys).toMatch(/never invent/i);
  });

  it('grounds the answer prompt in the résumé and includes the question', () => {
    const prompt = buildApplicationAnswerPrompt({
      question: 'Why do you want to work at this company?',
      resume: RESUME_FOR_GROUNDING,
      jobAd: 'Backend role needing Kubernetes and Go',
      meta: { ...META, topRequirements: ['React', 'Kubernetes'] },
    });
    expect(prompt).toContain('<candidate_resume>');
    expect(prompt).toContain('Why do you want to work at this company?');
    // Reuses the grounding split: a résumé-absent requirement is flagged ABSENT.
    expect(prompt).toMatch(/ABSENT/);
    // No brief provided → no research block.
    expect(prompt).not.toContain('<company_research>');
  });

  it('folds a company brief into a fenced, untrusted block when provided', () => {
    const brief = 'Globex is a logistics company expanding into the EU market.';
    const prompt = buildApplicationAnswerPrompt({
      question: 'Why this company?',
      resume: RESUME_FOR_GROUNDING,
      jobAd: 'A role',
      meta: META,
      companyBrief: brief,
    });
    expect(prompt).toContain('<company_research>');
    expect(prompt).toContain(brief);
    expect(prompt).toMatch(/untrusted/i);
    expect(prompt).toMatch(/ignore any instructions/i);
  });

  it('is market-aware and uses applicant details for logistics answers', () => {
    const prompt = buildApplicationAnswerPrompt({
      question: 'What are your salary expectations?',
      resume: RESUME_FOR_GROUNDING,
      jobAd: 'A role',
      meta: META,
      market: 'de',
      applicant: { salaryExpectation: '€70,000', noticePeriod: '3 months' },
    });
    expect(prompt).toContain('Market: Germany');
    expect(prompt).toContain('<applicant_details>');
    expect(prompt).toContain('€70,000');
    expect(prompt).toContain('3 months');
  });

  it('system prompt forbids fabricating logistics and allows research where it helps', () => {
    const sys = buildApplicationAnswerSystemPrompt();
    expect(sys).toMatch(/<applicant_details>/);
    expect(sys).toMatch(/never invent a number or date/i);
    expect(sys).toMatch(/company_research/i);
  });

  it('no longer blanket-forbids a salary number, but still forbids fabricating one', () => {
    const sys = buildApplicationAnswerSystemPrompt();
    expect(sys).toMatch(/<applicant_details>/);
    // Condition-first wording (safety hardening): the "only when" gate leads,
    // so a small local model can't over-weight "don't hedge" before checking
    // whether a salary expectation is even present.
    expect(sys).toMatch(/only when <applicant_details> lists a salary expectation/i);
    // The gate also requires an actual number, not just any stated expectation
    // (a free-text "competitive"/"negotiable" must not trigger a fabricated figure).
    expect(sys).toMatch(/contains an actual number/i);
    expect(sys).toMatch(/without hedging/i);
    expect(sys).toMatch(/never state a number/i);
    expect(sys).toMatch(/never fabricate a number/i);
    // Other logistics (dates/notice) keep the blanket no-invention rule.
    expect(sys).toMatch(/never invent a number or date/i);
  });

  it('appends the salary question guidance when passed, but not for other questions', () => {
    const salaryEntry = APPLICATION_QUESTIONS.find((q) => q.id === 'salary');
    const guidance = salaryEntry?.guidance;
    expect(guidance).toBeTruthy();

    const withGuidance = buildApplicationAnswerPrompt({
      question: salaryEntry?.question ?? '',
      resume: RESUME_FOR_GROUNDING,
      jobAd: 'A role',
      meta: META,
      guidance,
    });
    expect(withGuidance).toContain(guidance ?? '');

    // A non-salary registry entry has no guidance at all, and a caller that
    // omits the param renders no guidance line.
    const other = APPLICATION_QUESTIONS.find((q) => q.id === 'why-company');
    expect(other?.guidance).toBeUndefined();
    const withoutGuidance = buildApplicationAnswerPrompt({
      question: other?.question ?? '',
      resume: RESUME_FOR_GROUNDING,
      jobAd: 'A role',
      meta: META,
    });
    expect(withoutGuidance).not.toContain('Number:');
  });

  it('the salary guidance itself never invents a number and omits the line when ungrounded', () => {
    const salaryEntry = APPLICATION_QUESTIONS.find((q) => q.id === 'salary');
    expect(salaryEntry?.guidance).toMatch(/never invent a figure/i);
    expect(salaryEntry?.guidance).toMatch(/omit that final line/i);
    // Non-committal path: no stated expectation -> stay non-committal AND
    // omit the "Number:" line, in one instruction (not just two separate
    // claims that could drift apart under a future edit).
    expect(salaryEntry?.guidance).toMatch(/stay non-committal and omit that final line/i);
    // A present-but-non-numeric expectation ("competitive", "negotiable", "DOE")
    // must fall into the SAME omit-the-line path as no expectation at all.
    expect(salaryEntry?.guidance).toMatch(/contains no number/i);
    // Range -> single Number line is pinned deterministically to the upper
    // bound of the applicant's own stated range (grounded, not fabricated).
    expect(salaryEntry?.guidance).toMatch(/upper bound/i);
  });
});

describe('applicant preferences block', () => {
  it('fences stated preferences and forbids fabrication', () => {
    const block = buildApplicantDetailsBlock({
      salaryExpectation: '€70,000',
      earliestStartDate: '1 March 2026',
    });
    expect(block).toContain('<applicant_details>');
    expect(block).toContain('€70,000');
    expect(block).toContain('1 March 2026');
    expect(block).toMatch(/never invent/i);
  });

  it('is empty when nothing is set (so prompts pay nothing)', () => {
    expect(buildApplicantDetailsBlock(undefined)).toBe('');
    expect(buildApplicantDetailsBlock({})).toBe('');
    expect(buildApplicantDetailsBlock({ salaryExpectation: '   ' })).toBe('');
  });

  it('cover letter folds applicant details in for market inclusions (DACH)', () => {
    const prompt = buildCoverLetterPrompt(
      RESUME_WITH_LINKS,
      'Job ad',
      META,
      'recruiter',
      'large',
      '',
      'de',
      { salaryExpectation: '€70,000', earliestStartDate: '1 March 2026' }
    );
    expect(prompt).toContain('<applicant_details>');
    expect(prompt).toContain('€70,000');
  });
});

describe('extractPlainText', () => {
  it('strips think blocks, markdown headers and inline code', () => {
    const raw = '<think>internal reasoning</think>\n# Heading\nSome text with `code` here.';
    const out = extractPlainText(raw);
    expect(out).not.toContain('<think>');
    expect(out).not.toContain('internal reasoning');
    expect(out).not.toContain('# Heading');
    expect(out).toContain('Heading');
    expect(out).not.toContain('`code`');
    expect(out).toContain('code');
  });

  it('strips XML wrapper tags echoed from the prompt', () => {
    const out = extractPlainText('<candidate_resume>body</candidate_resume>');
    expect(out).not.toContain('<candidate_resume>');
    expect(out).toContain('body');
  });

  it('reduces emphasis markers (triple/single asterisks collapse)', () => {
    // The single-italic pass also unwraps the inner pair of a bold run.
    expect(extractPlainText('***strong***')).toBe('*strong*');
    expect(extractPlainText('an *italic* word')).toBe('an italic word');
  });

  it('removes a fenced code block entirely (no orphaned backticks or code leak)', () => {
    // Regression: the inline-backtick pass used to consume the ``` fence markers
    // first, so the fenced regex could not match and the code body leaked.
    const out = extractPlainText('Intro.\n```\nconst x = 1;\n```\nOutro.');
    // The fence (and only the fence) is gone — surrounding prose is preserved.
    // The minimal reorder fix leaves the blank line where the fence stood; what
    // matters is no backticks survive and the code body does not leak.
    expect(out).not.toContain('```');
    expect(out).not.toContain('const x = 1;');
    expect(out).toContain('Intro.');
    expect(out).toContain('Outro.');
    expect(out.replace(/\n+/g, '\n')).toBe('Intro.\nOutro.');
  });

  it('strips a language-tagged fenced block too', () => {
    const out = extractPlainText('Before\n```ts\nlet y = 2;\n```\nAfter');
    expect(out).not.toContain('let y = 2;');
    expect(out).not.toContain('```');
    expect(out).toContain('Before');
    expect(out).toContain('After');
  });

  it('still strips inline single-backtick code spans', () => {
    const out = extractPlainText('Use the `npm install` command.');
    expect(out).toBe('Use the npm install command.');
    expect(out).not.toContain('`');
  });
});

describe('validateMetadata', () => {
  it('parses well-formed JSON and applies defaults', () => {
    const meta = validateMetadata('{"candidateName":"Jane","jobTitle":"Dev","jobAdLanguage":"de"}');
    expect(meta?.candidateName).toBe('Jane');
    expect(meta?.targetLanguage).toBe('de');
    expect(meta?.mismatch).toBe(true); // resumeLanguage defaults to en, jobAd is de
    expect(meta?.topRequirements).toEqual([]);
  });

  it('extracts JSON embedded in surrounding prose', () => {
    const meta = validateMetadata('Here you go: {"candidateName":"Bob"} done.');
    expect(meta?.candidateName).toBe('Bob');
  });

  it('returns null for unparseable input', () => {
    expect(validateMetadata('not json at all')).toBeNull();
  });

  it('extracts and upper-cases the job location + country', () => {
    const meta = validateMetadata(
      '{"candidateName":"Jane","jobAdLanguage":"en","jobLocation":"Munich, Germany","jobCountry":"de"}'
    );
    expect(meta?.jobLocation).toBe('Munich, Germany');
    expect(meta?.jobCountry).toBe('DE');
  });

  it('drops a malformed jobCountry that is not a 2-letter code', () => {
    const meta = validateMetadata('{"candidateName":"Jane","jobCountry":"Germany"}');
    expect(meta?.jobCountry).toBe('');
    expect(meta?.jobLocation).toBe('');
  });
});
