import { describe, expect, it } from 'vitest';

import type { GenerationMeta } from '../modes/index.js';
import { buildJobAdSummaryPrompt, buildJobAdSummarySystemPrompt } from './job-ad-summary.js';

const META: GenerationMeta = {
  resumeLanguage: 'en',
  jobAdLanguage: 'de',
  mismatch: false,
  candidateName: 'John Doe',
  jobTitle: 'Senior Engineer',
  companyName: 'Acme',
  targetLanguage: 'de',
  topRequirements: [],
};

describe('buildJobAdSummarySystemPrompt', () => {
  it('forbids fabrication and pins output to the ad language', () => {
    const sys = buildJobAdSummarySystemPrompt();
    expect(sys).toMatch(/never fabricate|do not fabricate/i);
    // No facts invented beyond the ad.
    expect(sys).toMatch(/not present in the ad|only what the ad/i);
    // Output follows the ad's own language, not English.
    expect(sys).toMatch(/ad's own language/i);
    expect(sys).toMatch(/markdown/i);
  });
});

describe('buildJobAdSummaryPrompt', () => {
  it('fences the job ad and asks for the section labels (no résumé required)', () => {
    const jobAd = 'Senior Backend Engineer at Acme. You will build payment APIs in Rust.';
    // Called with ONLY a job ad — no résumé argument exists.
    const prompt = buildJobAdSummaryPrompt(jobAd);

    expect(prompt).toContain('<job_ad>');
    expect(prompt).toContain(jobAd);

    // All five section labels land in the prompt.
    expect(prompt).toContain('**Role & seniority**');
    expect(prompt).toContain('**Key responsibilities**');
    expect(prompt).toContain('**Must-haves**');
    expect(prompt).toContain('**Nice-to-haves**');
    expect(prompt).toContain('**Comp & logistics**');

    // It instructs a scannable digest, not a re-print, and omits empty sections.
    expect(prompt).toMatch(/OMIT any the ad does not cover/i);

    // No résumé fence is ever produced.
    expect(prompt).not.toContain('<candidate_resume>');
  });

  it('uses the meta target language when meta is provided', () => {
    const prompt = buildJobAdSummaryPrompt('Stelle: Backend Engineer bei Acme.', META);
    expect(prompt).toMatch(/Write the digest in de/i);
  });

  it('follows meta.targetLanguage over the ad language when they differ', () => {
    // German ad, but the target language is English — the output-language
    // instruction must follow meta.targetLanguage (en), not the ad's language (de).
    const meta: GenerationMeta = { ...META, jobAdLanguage: 'de', targetLanguage: 'en' };
    const prompt = buildJobAdSummaryPrompt('Stelle: Backend Engineer bei Acme in Berlin.', meta);
    expect(prompt).toMatch(/Write the digest in en/i);
    expect(prompt).not.toMatch(/Write the digest in de/i);
  });

  it('falls back to the ad language when no meta is given', () => {
    const prompt = buildJobAdSummaryPrompt('Job ad text.');
    expect(prompt).toMatch(/Write the digest in the ad's own language/i);
  });
});
