import { describe, expect, it } from 'vitest';

import {
  buildCoverLetterPrompt,
  buildCoverLetterSystemPrompt,
  buildMetadataPrompt,
  buildResumePrompt,
  buildResumeSystemPrompt,
  extractPlainText,
  type GenerationMeta,
  getLinkMap,
  injectLinksIntoGeneratedText,
  MODES,
  parseLinksFromResume,
  validateMetadata,
} from './generate';

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
  it('maps profile labels to URLs and skips email + non-profile links', () => {
    const map = getLinkMap(RESUME_WITH_LINKS);
    expect(map.LinkedIn).toBe('https://linkedin.com/in/johndoe');
    expect(map.GitHub).toBe('https://github.com/johndoe');
    expect(map.Email).toBeUndefined();
    expect(map.Personal).toBeUndefined();
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
});

describe('parseLinksFromResume', () => {
  it('extracts a clean email and profile labels', () => {
    const { block, cleanEmail } = parseLinksFromResume(RESUME_WITH_LINKS);
    expect(cleanEmail).toBe('john@example.com');
    expect(block).toContain('LinkedIn');
    expect(block).toContain('GitHub');
  });

  it('returns empty result when there is no reference block', () => {
    expect(parseLinksFromResume('No block here')).toEqual({ block: '', cleanEmail: '' });
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
});

describe('buildCoverLetterSystemPrompt', () => {
  it('returns a detailed prompt for large models', () => {
    const prompt = buildCoverLetterSystemPrompt('recruiter');
    expect(prompt).toContain('WHAT KILLS COVER LETTERS');
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
});
