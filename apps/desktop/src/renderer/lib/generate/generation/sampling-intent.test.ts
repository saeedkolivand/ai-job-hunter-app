import { describe, expect, it, vi } from 'vitest';

import { usePreferencesStore } from '@/store/preferences-store';

import {
  extractMetadata,
  generateApplicationAnswer,
  generateApplicationEmail,
  generateCoverLetter,
  generateReferral,
  generateReferralImprove,
  generateResume,
  rewriteSelection,
} from './generation';
import { installGenerationHooks, register, setActive, streamThrough } from './test-support';

installGenerationHooks();

describe('generation intent wiring (renderer states intent, adapter picks numbers)', () => {
  // The renderer NEVER sends topP/frequencyPenalty/presencePenalty/
  // repeatPenalty anymore — each provider adapter maps `(model, intent)` to
  // its own numbers, or none at all (`AiProvider::sampling_profile`,
  // `commands/ai_provider/mod.rs`). These tests pin the INTENT each surface
  // sends and that no renderer-side sampling number ever rides along.
  const META = {
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    candidateName: 'X',
    jobTitle: 'Y',
    companyName: 'Z',
    targetLanguage: 'en',
    topRequirements: [],
  };

  const argOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    return call?.[0] as {
      intent?: string;
      topP?: number;
      frequencyPenalty?: number;
      presencePenalty?: number;
      repeatPenalty?: number;
    };
  };

  const expectNoLegacySamplingFields = (client: ReturnType<typeof register>) => {
    const arg = argOf(client);
    expect(arg.topP).toBeUndefined();
    expect(arg.frequencyPenalty).toBeUndefined();
    expect(arg.presencePenalty).toBeUndefined();
    expect(arg.repeatPenalty).toBeUndefined();
  };

  it('cover letter sends the prose_grounded intent (asserts real résumé achievements to an employer), no legacy sampling fields', async () => {
    const client = register();
    await streamThrough(
      generateCoverLetter('My resume', 'Job ad', META, 'recruiter', 'llama3', vi.fn()),
      'Dear Hiring Team.'
    );
    expect(argOf(client).intent).toBe('prose_grounded');
    expectNoLegacySamplingFields(client);
  });

  it('application answer generation sends the prose_grounded intent (résumé-grounded, no-fabrication surface)', async () => {
    const client = register();
    await streamThrough(
      generateApplicationAnswer({
        question: 'Why do you want to work here?',
        resume: 'My resume',
        jobAd: 'Backend role at Acme',
        meta: META,
        model: 'llama3',
      }),
      'Because I love building things.'
    );
    expect(argOf(client).intent).toBe('prose_grounded');
    expectNoLegacySamplingFields(client);
  });

  it('referral message sends prose_grounded (candidate factual claims to a real person)', async () => {
    const client = register();
    await streamThrough(
      generateReferral({
        personName: 'Jamie',
        companyName: 'Acme',
        jobTitle: 'Engineer',
        resume: 'My resume',
        format: 'connection_note',
        model: 'llama3',
      }),
      'Hi Jamie,'
    );
    expect(argOf(client).intent).toBe('prose_grounded');
    expectNoLegacySamplingFields(client);
  });

  it('referral improve sends prose_grounded (mirrors generateReferral, same no-fabrication contract)', async () => {
    const client = register();
    await streamThrough(
      generateReferralImprove({
        personName: 'Jamie',
        companyName: 'Acme',
        jobTitle: 'Engineer',
        resume: 'My resume',
        draft: 'Hi Jamie, I noticed the Engineer opening.',
        instruction: 'Make it shorter',
        format: 'connection_note',
        model: 'llama3',
      }),
      'Hi Jamie,'
    );
    expect(argOf(client).intent).toBe('prose_grounded');
    expectNoLegacySamplingFields(client);
  });

  it('application email sends prose_grounded (candidate factual claims to a real employer)', async () => {
    const client = register();
    await streamThrough(
      generateApplicationEmail({
        resume: 'My resume',
        jobAd: 'Backend role at Acme',
        meta: META,
        model: 'llama3',
      }),
      'Subject: Application\nDear Hiring Team,'
    );
    expect(argOf(client).intent).toBe('prose_grounded');
    expectNoLegacySamplingFields(client);
  });

  it('resume generation sends the deterministic intent (protects exact ATS keyword repetition)', async () => {
    const client = register();
    await streamThrough(
      generateResume('My resume', 'Job ad', META, 'ats', 'llama3', vi.fn()),
      'RESUME CONTENT'
    );
    expect(argOf(client).intent).toBe('deterministic');
    expectNoLegacySamplingFields(client);
  });

  it('metadata extraction (analysis) sends the deterministic intent', async () => {
    const client = register();
    await streamThrough(extractMetadata('My resume', 'Job ad', 'llama3'), '{}');
    expect(argOf(client).intent).toBe('deterministic');
    expectNoLegacySamplingFields(client);
  });
});

describe('rewriteSelection intent (regression: previously bypassed resolveTemperature entirely)', () => {
  // `rewriteSelection` sent a bare `0.3` literal that never went through
  // `resolveTemperature`/`resolveSampling` — the per-model Ollama override
  // never applied to inline rewrites. Now routed through the same
  // per-`docType` Ollama-override lookup every other surface uses, but
  // ALWAYS with `deterministic` intent, regardless of docType: a surgical
  // span edit ("tighten this sentence") must never inherit a prose/
  // detector-resistance profile just because the surrounding document does
  // — that would reintroduce drift/fabrication risk into exactly the span
  // the user is deliberately hand-shaping.
  const argOf = (client: ReturnType<typeof register>) => {
    const call = (client.ai.generatePipeline as ReturnType<typeof vi.fn>).mock.calls[0];
    return call?.[0] as { temperature?: number; intent?: string };
  };

  it.each(['resume', 'cover-letter', 'application-answer', 'email'] as const)(
    '%s rewrite always sends the deterministic intent',
    async (docType) => {
      const client = register();
      await streamThrough(
        rewriteSelection({
          selection: 'I led the migration',
          instruction: 'Tighten this',
          before: '',
          after: '',
          docType,
          model: 'llama3',
        }),
        'I led a critical migration'
      );
      expect(argOf(client).intent).toBe('deterministic');
    }
  );

  it('honors the per-step Ollama temperature override for the resolved step', async () => {
    setActive('ollama', 'llama3');
    usePreferencesStore.setState({
      aiProviderConfig: {
        activeProvider: 'ollama',
        providers: {
          ollama: { model: 'llama3', modelLimits: { llama3: { temperature: { cover: 0.9 } } } },
        },
      },
    });
    const client = register();
    await streamThrough(
      rewriteSelection({
        selection: 'I am writing to apply',
        instruction: 'Make it warmer',
        before: '',
        after: '',
        docType: 'cover-letter',
        model: 'llama3',
      }),
      'I am delighted to apply'
    );
    expect(argOf(client).temperature).toBeCloseTo(0.9);
  });
});
