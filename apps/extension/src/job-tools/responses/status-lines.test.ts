/**
 * Unit tests for the Import / Fill status-line decisions, the copy-field fallback
 * projection and the trust gate (`responses.ts`; re-exported from `job-tools.ts`).
 * No DOM, no mounting.
 */

import { describe, expect, it } from 'vitest';

import {
  buildProfileFallbackFields,
  isPageTrusted,
  resolveFillResponse,
  resolveImportResponse,
} from '../responses';
import { answerState, failure, reply } from '../test-support';

const UNEXPECTED = 'Unexpected response — please retry.';
const LANDING = 'Open AI Job Hunter → Applications to view it.';

describe('resolveImportResponse', () => {
  it.each([
    [
      'returns an error message when ok=false',
      failure('Bridge unavailable.'),
      false,
      'Bridge unavailable.',
      'err',
    ],
    [
      'returns the unexpected-response error message when kind is not import',
      reply('token'),
      false,
      UNEXPECTED,
      'err',
    ],
    [
      'returns the result error text when the import result carries an error',
      reply('import', { result: { error: 'Desktop app rejected the job URL.' } }),
      false,
      'Desktop app rejected the job URL.',
      'err',
    ],
    [
      'names the imported job and points to where it landed when a title is present',
      reply('import', {
        result: { applicationId: 'app-123', status: 'saved', title: 'Senior Rust Engineer' },
      }),
      false,
      `Imported “Senior Rust Engineer”. ${LANDING}`,
      'ok',
    ],
    [
      'falls back to a generic success + landing hint when no title is present',
      reply('import', { result: { applicationId: 'app-456' } }),
      false,
      `Imported. ${LANDING}`,
      'ok',
    ],
    [
      'shows a partial message with title when partial=true',
      reply('import', {
        result: { applicationId: 'app-789', title: 'Frontend Engineer', partial: true },
      }),
      false,
      "Imported “Frontend Engineer” — couldn't read the description. Open AI Job Hunter → Applications to paste it.",
      'ok',
    ],
    [
      'surfaces an "already tracked" transparency message when the matched row is already past saved and the checkbox was unticked',
      reply('import', {
        result: { applicationId: 'app-existing', status: 'applied', title: 'Backend Engineer' },
      }),
      false,
      `“Backend Engineer” is already tracked as Applied — status unchanged. ${LANDING}`,
      'ok',
    ],
    [
      'does not show the transparency message when the checkbox was ticked, even for a non-saved status',
      reply('import', {
        result: { applicationId: 'app-2', status: 'applied', title: 'DevOps Engineer' },
      }),
      true,
      `Imported “DevOps Engineer”. ${LANDING}`,
      'ok',
    ],
    [
      'appends the percent-fit suffix when matchScore is present',
      reply('import', {
        result: {
          applicationId: 'app-score',
          status: 'saved',
          title: 'Rust Engineer',
          matchScore: 71.6,
        },
      }),
      false,
      `Imported “Rust Engineer”. ${LANDING} — 72% fit.`,
      'ok',
    ],
  ])('%s', (_name, res, requestedApplied, expectedText, expectedTone) => {
    const { text, tone } = resolveImportResponse(res, requestedApplied);
    expect(tone).toBe(expectedTone);
    expect(text).toBe(expectedText);
  });
});

describe('resolveFillResponse', () => {
  const summary = (over: Record<string, unknown>) =>
    reply('fill', { summary: { filled: [], nameSplit: null, filledNothing: false, ...over } });

  it.each([
    [
      'surfaces the desktop refusal as an error',
      failure('Autofill is off.'),
      'Autofill is off.',
      'err',
    ],
    [
      'reports the no-match case as a benign message, not an error',
      summary({ filledNothing: true }),
      'No matchable fields found on this page.',
      'ok',
    ],
    [
      'summarises the filled count and points the user at the page',
      summary({
        filled: [
          { key: 'email', label: 'Email', count: 2 },
          { key: 'phone', label: 'Phone', count: 1 },
        ],
      }),
      'Filled 3 fields — review them on the page.',
      'ok',
    ],
    [
      'flags the name-split guess in the confirmation',
      summary({
        filled: [{ key: 'firstName', label: 'First name', count: 1 }],
        nameSplit: { first: 'Saeed', last: 'Kolivand' },
      }),
      'Filled 1 field — review them on the page (name split is a guess — verify).',
      'ok',
    ],
    [
      'adds the skipped-ambiguous count (singular)',
      summary({ filled: [{ key: 'email', label: 'Email', count: 1 }], skippedAmbiguous: 1 }),
      'Filled 1 field — review them on the page. 1 field skipped (ambiguous match).',
      'ok',
    ],
    [
      'adds the skipped-ambiguous count (plural)',
      summary({ filled: [{ key: 'email', label: 'Email', count: 1 }], skippedAmbiguous: 3 }),
      'Filled 1 field — review them on the page. 3 fields skipped (ambiguous match).',
      'ok',
    ],
  ])('%s', (_name, res, expectedText, expectedTone) => {
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe(expectedTone);
    expect(text).toBe(expectedText);
  });
});

describe('buildProfileFallbackFields', () => {
  it('returns an empty list on a refusal/failure (error set)', () => {
    expect(buildProfileFallbackFields({ error: 'Not paired.', fullName: 'Ada' })).toEqual([]);
  });

  it('returns an empty list for an empty profile', () => {
    expect(buildProfileFallbackFields({})).toEqual([]);
  });

  it('omits blank/whitespace-only fields, includes only populated ones in order', () => {
    expect(
      buildProfileFallbackFields({ fullName: 'Ada Lovelace', email: '  ', phone: '555-1234' })
    ).toEqual([
      { label: 'Name', value: 'Ada Lovelace' },
      { label: 'Phone', value: '555-1234' },
    ]);
  });

  it('appends each extraLink under its own label', () => {
    expect(
      buildProfileFallbackFields({
        email: 'ada@example.com',
        extraLinks: [{ label: 'Portfolio', url: 'https://ada.dev' }],
      })
    ).toEqual([
      { label: 'Email', value: 'ada@example.com' },
      { label: 'Portfolio', value: 'https://ada.dev' },
    ]);
  });
});

describe('isPageTrusted', () => {
  it.each([
    ['is untrusted when no record exists for the tab', null, false],
    ['is untrusted for a record whose page has changed', answerState({ pageChanged: true }), false],
    [
      'is trusted only for an existing record with pageChanged:false',
      answerState({ pageChanged: false }),
      true,
    ],
  ])('%s', (_name, state, trusted) => {
    expect(isPageTrusted(state)).toBe(trusted);
  });
});
