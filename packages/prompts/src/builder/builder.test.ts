import { describe, expect, it } from 'vitest';

import type { GenerationMeta } from '../generate/modes.js';
import {
  buildBuilderSystemPrompt,
  buildInterviewResumePrompt,
  type InterviewAnswers,
  renderInterviewAnswers,
} from './index.js';

const META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'en',
  mismatch: false,
  candidateName: 'Jane Doe',
  jobTitle: '',
  companyName: '',
  targetLanguage: 'en',
  topRequirements: [],
};

const ANSWERS: InterviewAnswers = {
  fullName: 'Jane Doe',
  headline: 'Senior Backend Engineer',
  experience: [
    {
      title: 'Senior Engineer',
      company: 'Acme',
      location: 'Berlin',
      startDate: 'Jan 2021',
      endDate: '',
      current: true,
      bullets: ['Built a payments service handling 200k requests/day'],
    },
  ],
  education: [{ degree: 'MSc Computer Science', institution: 'TU Munich', endDate: '2018' }],
  skills: ['Python', 'PostgreSQL', 'AWS'],
};

describe('renderInterviewAnswers', () => {
  it('renders core sections and omits empty optional ones', () => {
    const out = renderInterviewAnswers(ANSWERS);
    expect(out).toContain('NAME: Jane Doe');
    expect(out).toContain('HEADLINE: Senior Backend Engineer');
    expect(out).toContain('EXPERIENCE:');
    expect(out).toContain('Senior Engineer @ Acme (Berlin)');
    expect(out).toContain('EDUCATION:');
    expect(out).toContain('SKILLS: Python, PostgreSQL, AWS');
    // No optional sections were provided.
    expect(out).not.toContain('PROJECTS:');
    expect(out).not.toContain('PUBLICATIONS:');
    expect(out).not.toContain('LANGUAGES:');
  });

  it('renders a current role as "Present"', () => {
    expect(renderInterviewAnswers(ANSWERS)).toContain('Jan 2021 – Present');
  });

  it('keeps a candidate-written summary verbatim', () => {
    const out = renderInterviewAnswers({ ...ANSWERS, summary: 'A hands-on platform engineer.' });
    expect(out).toContain('SUMMARY (candidate-written — keep its substance):');
    expect(out).toContain('A hands-on platform engineer.');
  });

  it('keeps project and publication links inline (#18)', () => {
    const out = renderInterviewAnswers({
      ...ANSWERS,
      projects: [{ name: 'OSS Tool', description: 'A CLI', link: 'https://github.com/x/y' }],
      publications: [
        { title: 'On Caches', venue: 'JACM', year: '2022', link: 'https://doi.org/10.1/abc' },
      ],
    });
    expect(out).toContain('PROJECTS:');
    expect(out).toContain('[OSS Tool](https://github.com/x/y)');
    expect(out).toContain('PUBLICATIONS:');
    expect(out).toContain('[On Caches](https://doi.org/10.1/abc)');
  });

  it('renders awards, volunteering, languages, and certifications when present', () => {
    const out = renderInterviewAnswers({
      ...ANSWERS,
      awards: [{ title: 'Hackathon winner', year: '2020' }],
      volunteer: [{ title: 'Mentor', detail: 'CoderDojo' }],
      languages: ['English (native)', 'German (B2)'],
      certifications: ['AWS Solutions Architect'],
    });
    expect(out).toContain('AWARDS:');
    expect(out).toContain('VOLUNTEERING:');
    expect(out).toContain('LANGUAGES: English (native), German (B2)');
    expect(out).toContain('CERTIFICATIONS: AWS Solutions Architect');
  });
});

describe('buildBuilderSystemPrompt', () => {
  it('enforces no-fabrication grounded in <interview_answers> across tiers', () => {
    for (const tier of ['large', 'medium', 'small'] as const) {
      const sys = buildBuilderSystemPrompt(tier);
      expect(sys).toMatch(/interview_answers/);
      expect(sys.toLowerCase()).toContain('never invent');
      // The contact line is delegated to the export's saved profile.
      expect(sys.toLowerCase()).toContain('contact');
    }
  });
});

describe('buildInterviewResumePrompt', () => {
  it('grounds on the answers and asks to derive a summary when none given', () => {
    const user = buildInterviewResumePrompt(ANSWERS, META);
    expect(user).toContain('<interview_answers>');
    expect(user).toContain('Senior Engineer @ Acme');
    expect(user).toMatch(/derived strictly from the answers/);
    expect(user).toContain('Write in en');
  });

  it('respects a candidate-written summary instead of deriving one', () => {
    const user = buildInterviewResumePrompt({ ...ANSWERS, summary: 'A pragmatic engineer.' }, META);
    expect(user).toMatch(/keep its substance/);
  });

  it('uses the target market section headers (de)', () => {
    const user = buildInterviewResumePrompt(ANSWERS, { ...META, targetLanguage: 'de' });
    expect(user).toContain('Berufserfahrung');
    expect(user).toContain('Write in de');
  });

  it('folds in emphasis directives only when set (#15 future-proofing)', () => {
    const plain = buildInterviewResumePrompt(ANSWERS, META);
    expect(plain).not.toContain('user-selected biases');
    const biased = buildInterviewResumePrompt(ANSWERS, { ...META, emphasis: ['quantify'] });
    expect(biased).toContain('user-selected biases');
  });
});
