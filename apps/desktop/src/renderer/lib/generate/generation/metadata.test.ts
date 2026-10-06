import { describe, expect, it } from 'vitest';

import { extractMetadata } from './generation';
import { installGenerationHooks, register, streamThrough } from './test-support';

installGenerationHooks();

describe('extractMetadata', () => {
  it('parses streamed metadata JSON and overrides languages client-side', async () => {
    register();
    const meta = await streamThrough(
      extractMetadata('Professional Summary\nJane Smith resume', 'A React role', 'llama3'),
      '{"candidateName":"Jane Smith","jobTitle":"Frontend Engineer","companyName":"Acme"}'
    );
    expect(meta.candidateName).toBe('Jane Smith');
    expect(meta.jobTitle).toBe('Frontend Engineer');
    expect(meta).toHaveProperty('mismatch');
  });

  it('falls back to regex extraction when the model returns no JSON', async () => {
    register();
    const resume = 'John Doe\nsoftware engineer with 10 years experience.';
    const jobAd = 'Position: Senior Engineer\nCompany: Acme';
    const meta = await streamThrough(
      extractMetadata(resume, jobAd, 'llama3'),
      'sorry, no json available'
    );
    expect(meta.candidateName).toBe('John Doe');
    expect(meta.jobTitle).toBe('Senior Engineer');
    expect(meta.companyName).toBe('Acme');
  });

  // Defect B (cross-language generation fix): the target is who we're
  // writing FOR — the job ad's language — never whatever language the
  // candidate's existing résumé happens to be written in. An English résumé
  // applying to a German ad must not be pinned to English. Known-reliable
  // franc fixtures, lifted verbatim from `packages/shared/src/language-detection.test.ts`
  // (also reused by `useTailorPipeline.test.ts`).
  describe('targetLanguage targets the job ad, not the source résumé (Defect B)', () => {
    const ENGLISH_RESUME =
      'Experienced software engineer with a strong background in building scalable web applications and distributed backend systems for large organisations.';
    const GERMAN_JOB_AD =
      'Erfahrener Softwareentwickler mit fundierten Kenntnissen in der Entwicklung skalierbarer Webanwendungen und verteilter Backend-Systeme für große Unternehmen.';

    it('the heuristic fallback (model returned no JSON) targets the ad, not the résumé', async () => {
      register();
      const meta = await streamThrough(
        extractMetadata(ENGLISH_RESUME, GERMAN_JOB_AD, 'llama3'),
        'sorry, no json available'
      );
      // `targetLanguage` must be the ISO CODE ('de'), not the display NAME
      // ('German') `resumeLanguage`/`jobAdLanguage` carry — it is persisted
      // verbatim to `ai_generations.target_language` and read back by Rust's
      // `normalize_language`, which takes the first two alphanumeric chars:
      // 'German' silently becomes 'ge', matching no language arm.
      // Mutation: revert `targetLanguage: clientSideDetection.jobAd` to
      // `.jobAdName` (generation.ts) → this goes red ('German' instead of
      // 'de').
      expect(meta.targetLanguage).toBe('de');
      expect(meta.resumeLanguage).toBe('English');
      expect(meta.jobAdLanguage).toBe('German');
    });

    // The non-heuristic (model-JSON) path was ALREADY correct — it never
    // overrides `targetLanguage`, which `validateMetadata` already sets to
    // the model's own `jobAdLanguage` (`metadata.ts:211`). Pinned here so a
    // future change can't silently reintroduce the résumé-language bug on
    // this path too.
    it('the non-heuristic (model JSON) path also targets the ad, not the résumé', async () => {
      register();
      const meta = await streamThrough(
        extractMetadata(ENGLISH_RESUME, GERMAN_JOB_AD, 'llama3'),
        '{"candidateName":"X","jobTitle":"Y","companyName":"Z","resumeLanguage":"en","jobAdLanguage":"de"}'
      );
      expect(meta.targetLanguage).toBe('de');
    });
  });
});
