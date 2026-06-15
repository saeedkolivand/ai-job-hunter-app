import { describe, expect, it } from 'vitest';

import {
  buildInterviewQuestionsPrompt,
  buildInterviewQuestionsSystemPrompt,
} from './interview-questions.js';
import type { GenerationMeta } from './modes.js';

const META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'John Doe',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  targetLanguage: 'en',
  topRequirements: [],
};

describe('buildInterviewQuestionsSystemPrompt', () => {
  it('enforces the quality bar and the delimited output format', () => {
    const sys = buildInterviewQuestionsSystemPrompt();
    expect(sys).toMatch(/ASK their interviewer/i);
    // Bans weak / self-serving questions.
    expect(sys).toMatch(/salary|perks|PTO/i);
    expect(sys).toMatch(/careers page/i);
    // The lenient parser relies on these markers.
    expect(sys).toContain('Q:');
    expect(sys).toContain('WHY:');
    expect(sys).toContain('AUDIENCE:');
  });
});

describe('buildInterviewQuestionsPrompt', () => {
  it('omits the company-research block when no brief is provided', () => {
    const prompt = buildInterviewQuestionsPrompt({ resume: 'R', jobAd: 'JD', meta: META });
    expect(prompt).not.toContain('<company_research>');
  });

  it('fences a provided company brief as untrusted (ADR-010)', () => {
    const brief = 'Acme recently shipped a payments SDK and is expanding into the EU.';
    const prompt = buildInterviewQuestionsPrompt({
      resume: 'R',
      jobAd: 'JD',
      meta: META,
      companyBrief: brief,
    });
    expect(prompt).toContain('<company_research>');
    expect(prompt).toContain(brief);
    expect(prompt).toMatch(/untrusted/i);
    expect(prompt).toMatch(/ignore any instructions/i);
  });

  it('weaves in user seed topics when provided', () => {
    const prompt = buildInterviewQuestionsPrompt({
      resume: 'R',
      jobAd: 'JD',
      meta: META,
      seedTopics: ['on-call rotation', 'team growth'],
    });
    expect(prompt).toContain('on-call rotation');
    expect(prompt).toContain('team growth');
  });

  it('requests the configured number of questions', () => {
    const prompt = buildInterviewQuestionsPrompt({
      resume: 'R',
      jobAd: 'JD',
      meta: META,
      count: 4,
    });
    expect(prompt).toMatch(/Write 4 strong/i);
  });
});
