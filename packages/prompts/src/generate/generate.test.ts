import { describe, expect, it } from 'vitest';

import urlLabels from '../fixtures/url-labels.json';
import {
  APPLICATION_QUESTIONS,
  buildApplicantDetailsBlock,
  buildApplicationAnswerPrompt,
  buildApplicationAnswerSystemPrompt,
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildGroundingBlock,
  buildMetadataPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  extractPlainText,
  type GenerationMeta,
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

  it('admits exactly one Website link and drops later non-platform URLs', () => {
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
    expect(map.Website).toBe('https://janedoe.dev'); // first non-platform wins
    expect(Object.values(map)).not.toContain('https://janeblog.example'); // 2nd dropped
    expect(Object.values(map)).not.toContain('mailto:jane@example.com'); // mailto dropped
  });

  it('returns an empty map when there is no reference block', () => {
    expect(getLinkMap('Just a plain resume with no separator')).toEqual({});
  });

  it('derives a friendly label when the anchor is a raw URL', () => {
    const resume = `Body\n---\n- [https://github.com/jane](https://github.com/jane)`;
    const map = getLinkMap(resume);
    expect(map.GitHub).toBe('https://github.com/jane');
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
