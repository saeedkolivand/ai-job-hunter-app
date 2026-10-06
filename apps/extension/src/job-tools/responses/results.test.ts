/**
 * Unit tests for the Check fit / Save answers / Stamp / fields-probe response
 * decisions (`responses.ts`; re-exported from `job-tools.ts`). No DOM, no mounting.
 */

import { describe, expect, it } from 'vitest';

import {
  resolveAnswersSaveResponse,
  resolveFieldsProbeResponse,
  resolveMatchLiveResponse,
  resolveStampResultsResponse,
} from '../responses';
import { failure, reply } from '../test-support';

describe('resolveFieldsProbeResponse', () => {
  it.each([
    [
      'returns both signals on success',
      reply('fieldsProbe', { hasFormFields: true, hasAnswerFields: false }),
      { showFormGroup: true, showAnswerTools: false },
    ],
    [
      'fails OPEN (both true) on a transport-level ok:false',
      failure('message channel closed'),
      { showFormGroup: true, showAnswerTools: true },
    ],
    [
      'fails OPEN (both true) for an unexpected response kind',
      reply('token'),
      { showFormGroup: true, showAnswerTools: true },
    ],
  ])('%s', (_name, res, expected) => {
    expect(resolveFieldsProbeResponse(res)).toEqual(expected);
  });
});

describe('resolveAnswersSaveResponse', () => {
  const saved = (over: Record<string, unknown>) =>
    reply('answersSave', {
      result: { ok: true, applicationId: 'app-1', saved: 0, skipped: 0, ...over },
    });

  it.each([
    [
      'names the job with title @ company and the saved count on success',
      saved({ saved: 7, skipped: 2, title: 'Backend Engineer', company: 'Acme' }),
      'Saved 7 answers to Backend Engineer @ Acme — 2 already recorded.',
      'ok',
    ],
    [
      'falls back to a generic "no new answers" message when saved and skipped are both 0',
      saved({}),
      'No new answers to save from this page.',
      'ok',
    ],
    [
      'shows a distinct "already recorded" message when saved is 0 but skipped is not',
      saved({ skipped: 3 }),
      'All 3 answers were already recorded.',
      'ok',
    ],
    [
      'surfaces the desktop refusal text when result.ok is false',
      reply('answersSave', {
        result: { ok: false, error: "couldn't find a saved job for this page" },
      }),
      "couldn't find a saved job for this page",
      'err',
    ],
  ])('%s', (_name, res, expectedText, expectedTone) => {
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe(expectedTone);
    expect(text).toBe(expectedText);
  });
});

describe('resolveMatchLiveResponse', () => {
  const matched = (over: Record<string, unknown> = {}) =>
    reply('matchLive', {
      result: {
        ok: true,
        combined: 71.6,
        ats: 60,
        gaps: [],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
        ...over,
      },
    });

  it('surfaces a transport-level error with null score fields', () => {
    const view = resolveMatchLiveResponse(failure('Desktop app not reachable.'));
    expect(view.tone).toBe('err');
    expect(view.score).toBeNull();
    expect(view.gaps).toEqual([]);
  });

  it('surfaces the desktop refusal text when result.ok is false', () => {
    const view = resolveMatchLiveResponse(
      reply('matchLive', {
        result: {
          ok: false,
          error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
        },
      })
    );
    expect(view.tone).toBe('err');
    expect(view.score).toBeNull();
  });

  it('renders the rounded score, source label, résumé name, and gaps on success', () => {
    const view = resolveMatchLiveResponse(matched({ gaps: ['kubernetes', 'terraform'] }));
    expect(view.tone).toBe('ok');
    expect(view.score).toBe(72);
    expect(view.scoreLabel).toBe('keyword coverage');
    expect(view.resumeName).toBe('My Resume');
    expect(view.gaps).toEqual(['kubernetes', 'terraform']);
    expect(view.text).toBe('72% fit against “My Resume”.');
  });

  it('passes the salary facts through verbatim (PR3), when present', () => {
    const salary = { posting: '€70,000–€90,000', expectation: '€80,000' };
    expect(resolveMatchLiveResponse(matched({ salary })).salary).toEqual(salary);
  });

  it('leaves salary undefined when the desktop omitted it', () => {
    expect(resolveMatchLiveResponse(matched({ combined: 50, ats: 50 })).salary).toBeUndefined();
  });
});

describe('resolveStampResultsResponse', () => {
  it.each([
    [
      'surfaces a transport-level error',
      failure('Not paired. Paste your pairing token first.'),
      'Not paired. Paste your pairing token first.',
      'err',
    ],
    [
      'renders the status line as ok, even for a desktop-side refusal (never a partial lie)',
      reply('stampResults', { stamped: 0, status: 'Too many job cards on this page.' }),
      'Too many job cards on this page.',
      'ok',
    ],
    [
      'renders the success count',
      reply('stampResults', { stamped: 3, status: 'Stamped 3 cards.' }),
      'Stamped 3 cards.',
      'ok',
    ],
  ])('%s', (_name, res, expectedText, expectedTone) => {
    const { text, tone } = resolveStampResultsResponse(res);
    expect(tone).toBe(expectedTone);
    expect(text).toBe(expectedText);
  });
});
