import { describe, expect, it, vi } from 'vitest';

import { generateCoverLetter, researchCompany } from './generation';
import { installGenerationHooks, register, streamThrough } from './test-support';

installGenerationHooks();

describe('generateCoverLetter', () => {
  it('returns the cleaned letter text', async () => {
    register();
    const onToken = vi.fn();
    const out = await streamThrough(
      generateCoverLetter(
        'My resume',
        'Job ad',
        {
          resumeLanguage: 'en',
          jobAdLanguage: 'en',
          mismatch: false,
          candidateName: 'X',
          jobTitle: 'Y',
          companyName: 'Z',
          targetLanguage: 'en',
          topRequirements: [],
        },
        'recruiter',
        'llama3',
        onToken
      ),
      'Dear Hiring Team, I am a great fit for this role and more.'
    );
    expect(out.text).toContain('Dear Hiring Team');
    // Research off → no brief on the result.
    expect(out.companyBrief).toBe('');
  });

  const COVER_META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };

  const registerWithResearch = (research: ReturnType<typeof vi.fn>) =>
    register({ ai: { researchCompany: research } });

  it('researches the company and folds the brief into the prompt when enabled', async () => {
    const research = vi
      .fn()
      .mockResolvedValue({ company: 'Acme', brief: 'Acme builds payment rails for SMBs.' });
    const client = registerWithResearch(research);

    const out = await streamThrough(
      generateCoverLetter(
        'My resume',
        'Job ad at Acme',
        COVER_META,
        'recruiter',
        'llama3',
        vi.fn(),
        'en',
        undefined,
        undefined,
        { researchCompany: true }
      ),
      'Dear Hiring Team, great fit.'
    );

    // The fetched brief is surfaced on the result so the caller can persist it.
    expect(out.companyBrief).toBe('Acme builds payment rails for SMBs.');
    expect(research).toHaveBeenCalledWith(expect.objectContaining({ jobAd: 'Job ad at Acme' }));
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('Acme builds payment rails'),
          }),
        ]),
      })
    );
  });

  it('skips research entirely when the flag is off (no extra call)', async () => {
    const research = vi.fn().mockResolvedValue({ company: '', brief: '' });
    registerWithResearch(research);

    await streamThrough(
      generateCoverLetter('My resume', 'Job ad', COVER_META, 'recruiter', 'llama3', vi.fn()),
      'Dear Hiring Team.'
    );

    expect(research).not.toHaveBeenCalled();
  });

  it('researchCompany degrades to an empty brief when the backend fails', async () => {
    registerWithResearch(vi.fn().mockRejectedValue(new Error('provider cannot search')));
    expect(await researchCompany('Job ad')).toBe('');
  });
});
