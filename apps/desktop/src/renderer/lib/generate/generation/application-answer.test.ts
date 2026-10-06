import { describe, expect, it } from 'vitest';

import { generateApplicationAnswer } from './generation';
import { installGenerationHooks, register, streamThrough } from './test-support';

installGenerationHooks();

describe('generateApplicationAnswer', () => {
  it('grounds an answer prompt with the question + brief and returns clean text', async () => {
    const client = register();

    const answer = await streamThrough(
      generateApplicationAnswer({
        question: 'Why do you want to work here?',
        resume: 'My resume: led a payments migration.',
        jobAd: 'Backend role at Acme',
        meta: {
          resumeLanguage: 'en',
          jobAdLanguage: 'en',
          mismatch: false,
          candidateName: 'X',
          jobTitle: 'Backend Engineer',
          companyName: 'Acme',
          targetLanguage: 'en',
          topRequirements: [],
        },
        model: 'llama3',
        companyBrief: 'Acme builds payment rails.',
      }),
      '<think>plan</think>I led a payments migration, which maps to your rails work.'
    );

    expect(answer).toContain('payments migration');
    expect(answer).not.toContain('<think>');
    // The question and the (untrusted) brief both reach the user prompt.
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('Why do you want to work here?'),
          }),
        ]),
      })
    );
    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<company_research>'),
          }),
        ]),
      })
    );
  });

  it('folds opt-in web-search notes into a fenced <web_search_notes> block', async () => {
    const client = register();

    await streamThrough(
      generateApplicationAnswer({
        question: 'Why do you want to work here?',
        resume: 'My resume: led a payments migration.',
        jobAd: 'Backend role at Acme',
        meta: {
          resumeLanguage: 'en',
          jobAdLanguage: 'en',
          mismatch: false,
          candidateName: 'X',
          jobTitle: 'Backend Engineer',
          companyName: 'Acme',
          targetLanguage: 'en',
          topRequirements: [],
        },
        model: 'llama3',
        webSearchNotes: 'Acme recently announced a new product line.',
      }),
      'I led a payments migration relevant to your new product line.'
    );

    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<web_search_notes>'),
          }),
        ]),
      })
    );
  });

  it('folds a market salary range into the prompt as a fenced <salary_context> block (C2)', async () => {
    const client = register();

    await streamThrough(
      generateApplicationAnswer({
        question: 'What are your salary expectations?',
        resume: 'My resume: led a payments migration.',
        jobAd: 'Backend role at Acme',
        meta: {
          resumeLanguage: 'en',
          jobAdLanguage: 'en',
          mismatch: false,
          candidateName: 'X',
          jobTitle: 'Backend Engineer',
          companyName: 'Acme',
          targetLanguage: 'en',
          topRequirements: [],
        },
        model: 'llama3',
        salaryRange: { min: 65000, max: 80000, currency: 'EUR' },
      }),
      'Open to discussing, given the market range. Number: 72500'
    );

    expect(client.ai.generatePipeline).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.stringContaining('<salary_context>'),
          }),
        ]),
      })
    );
  });
});
