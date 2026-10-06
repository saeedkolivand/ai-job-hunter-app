/**
 * useTailorPipeline — start() — the run request (id-wins, flags, market/today/researchCompany) and target-language wiring.
 * Mocks + render helpers live in `harness.ts` (see its header).
 */
import { beforeEach, describe, expect, it } from 'vitest';

import {
  detail,
  ENGLISH_JOB_AD,
  expectStartRequest,
  GERMAN_JOB_AD,
  mockNotify,
  record,
  render,
  resetHarness,
  sessionBus,
  startResumeRun,
} from './harness';

beforeEach(resetHarness);

describe('useTailorPipeline — start() builds the id-wins run request', () => {
  it.each([
    [
      'sends resumeId (and an empty resumeText) when the wizard résumé is doc-backed',
      { resume: 'the résumé text', resumeDocId: 'doc-42', outputType: 'both' as const },
      { resumeId: 'doc-42', resumeText: '' },
    ],
    [
      'sends resumeText (and an empty resumeId) when the résumé has no backing doc',
      { resume: 'pasted résumé text', outputType: 'both' as const },
      { resumeId: '', resumeText: 'pasted résumé text' },
    ],
  ])('%s', async (_name, values, expected) => {
    await expectStartRequest({}, expected, values);
  });

  // The reported bug, at its source. This used to assert `includeCoverLetter`
  // for 'resume' and 'cover' only — and never compared 'cover' against 'both',
  // which is the one pair that was broken: with no résumé flag on the wire, the
  // three-way choice collapsed onto ONE boolean and those two sent a
  // byte-identical request. Asserting the PAIR, for all three values, against
  // absolute literals is what makes that impossible to reintroduce.
  it.each([
    ['resume', { includeResume: true, includeCoverLetter: false }],
    ['cover', { includeResume: false, includeCoverLetter: true }],
    ['both', { includeResume: true, includeCoverLetter: true }],
  ] as const)('maps outputType=%s onto both document flags', async (outputType, flags) => {
    const { result } = render();
    await startResumeRun(result, { outputType });
    expect(sessionBus.start).toHaveBeenLastCalledWith(expect.objectContaining(flags));
  });

  it('never fabricates a jobUrl — sends exactly what it was given, including empty', async () => {
    await expectStartRequest({ jobUrl: '' }, { jobUrl: '' });
  });

  it('does not start a run when AI is unavailable or there is no job ad', async () => {
    const { result } = render({ canUse: false });
    await startResumeRun(result);
    expect(sessionBus.start).not.toHaveBeenCalled();
  });

  it('toasts a failed start — the session already set the persistent banner text', async () => {
    sessionBus.start.mockResolvedValueOnce(null);
    const { result } = render();
    await startResumeRun(result);
    expect(mockNotify.error).toHaveBeenCalledWith({ message: 'autopilot.apply.failed' });
  });

  it('does not toast a successful start', async () => {
    const { result } = render();
    await startResumeRun(result);
    expect(mockNotify.error).not.toHaveBeenCalled();
  });
});

// The gap this closes: `market`/`today`/`researchCompany` were added to
// `ResumePipelineRunSchema` (all `.default()`-ed) but `start()` never sent
// any of them, so they silently sat at their defaults ('intl', '', false)
// and the letter-market-conventions/date/company-research features they
// unlock were inert regardless of the wizard's own state. Real (unmocked)
// `detectLanguage` fixtures, lifted from the market describe block below.
describe('useTailorPipeline — start() sends market/today/researchCompany (letter-export contract)', () => {
  it('sends the German market for a German-language posting', async () => {
    await expectStartRequest({ jobDesc: GERMAN_JOB_AD }, { market: 'de' });
  });

  it('sends the US market for a US-located English posting', async () => {
    await expectStartRequest(
      { jobDesc: ENGLISH_JOB_AD, jobLocation: 'New York, NY, US' },
      { market: 'us' }
    );
  });

  it('sends a non-empty, German-formatted today for a German posting', async () => {
    // Derived with the SAME `toLocaleDateString` call `start()` uses — asserts
    // the shape/locale, not a hardcoded literal that would break tomorrow.
    const expectedToday = new Date().toLocaleDateString('de', {
      day: 'numeric',
      month: 'long',
      year: 'numeric',
    });
    expect(expectedToday).not.toBe('');
    await expectStartRequest({ jobDesc: GERMAN_JOB_AD }, { today: expectedToday });
  });

  it.each([true, false])('reflects researchCompany: %s from the wizard values', async (value) => {
    await expectStartRequest({}, { researchCompany: value }, { researchCompany: value });
  });
});

describe('useTailorPipeline — cancel', () => {
  it('forwards to the session', () => {
    const { result } = render();
    result.current.cancel();
    expect(sessionBus.cancel).toHaveBeenCalledTimes(1);
  });
});

describe('useTailorPipeline — targetLanguage precedence is wired end to end', () => {
  it('resolves targetLanguage from the previous generation even when jobDesc is empty (the failing regenerate condition)', async () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const generation = record({
      id: 'gen-1',
      targetLanguage: 'de',
      jobAdLanguage: 'en',
      coverLetterText: '',
    });

    const result = await expectStartRequest(
      { jobDesc: '', latestGeneration: generation },
      { targetLanguage: 'de' }
    );

    expect(result.current.meta?.targetLanguage).toBe('de');
  });

  it('detects targetLanguage from the job ad when there is no previous generation (first run)', async () => {
    await expectStartRequest(
      { jobDesc: GERMAN_JOB_AD, latestGeneration: undefined },
      { targetLanguage: 'de' }
    );
  });

  // The negative case: a guessed language must be neither PERSISTED nor
  // PREFERRED (owner decision). This is the "neither persisted" half —
  // `resolveTargetLanguage`'s "not confident" test above pins the "neither
  // preferred" half (a future run's tier 1 can only prefer what actually
  // reached the wire). Mutation: send `targetLanguage` (the resolved 'en')
  // instead of `wireTargetLanguage` in `start()` → this goes red.
  it('never sends a guessed language for persistence — the wire targetLanguage is empty when nothing was confident', async () => {
    await expectStartRequest({ jobDesc: '', latestGeneration: undefined }, { targetLanguage: '' });
  });

  // The hook's own `targetLanguageConfident` is the value the two sibling
  // save paths (`useApplicationAnswers`, `useInterviewQuestions`) key off of
  // to withhold a guessed language from THEIR persist calls — it must track
  // `resolveTargetLanguage`'s own verdict, not just default to `true`.
  it('exposes targetLanguageConfident: false alongside a guessed meta.targetLanguage', () => {
    sessionBus.detail = detail({ resumeText: 'RESUME' });
    const { result } = render({ jobDesc: '', latestGeneration: undefined });
    expect(result.current.targetLanguageConfident).toBe(false);
    expect(result.current.meta?.targetLanguage).toBe('en');
  });

  it('exposes targetLanguageConfident: true when the language was actually detected', () => {
    const { result } = render({ jobDesc: GERMAN_JOB_AD, latestGeneration: undefined });
    expect(result.current.targetLanguageConfident).toBe(true);
  });
});
