/**
 * jobs.boardSummary.health.neverWorkedSince — grammar (MEDIUM 3).
 *
 * `BoardSummaryChips.test.tsx` mocks `@ajh/translations` down to a
 * `t(k, opts) => "${k}:${opts.since}"` identity stub (see the file header
 * there), so it can never see what the composed English SENTENCE reads like.
 * `timeAgo` returns a RELATIVE phrase ("2 days ago", "yesterday"), and the key
 * used to read "has never worked here · failing for {{since}}" — which
 * composed to the ungrammatical "failing for 2 days ago" / "failing for
 * yesterday".
 *
 * This calls the REAL i18next instance `useTranslation()` resolves to (same
 * one `translations-parity.test.ts` uses unmocked) with the REAL `timeAgo`
 * output — the exact composition `healthDetail` in BoardSummaryChips.tsx
 * performs in production — so a regression back to the old copy fails here.
 */
import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

import { timeAgo } from '@/lib/time';

describe('jobs.boardSummary.health.neverWorkedSince — grammar', () => {
  const NOW = Date.parse('2026-01-15T12:00:00Z');

  it('reads grammatically at the 1-day boundary ("yesterday", not "for yesterday")', () => {
    const since = timeAgo(NOW - 24 * 60 * 60 * 1000, NOW, 'en');
    expect(since).toBe('yesterday');
    const text = i18n.t('jobs.boardSummary.health.neverWorkedSince', { since, lng: 'en' });
    expect(text).toBe('has never worked here · first failed yesterday');
  });

  it('reads grammatically for a multi-day-old failure ("2 days ago", not "for 2 days ago")', () => {
    const since = timeAgo(NOW - 2 * 24 * 60 * 60 * 1000, NOW, 'en');
    expect(since).toBe('2 days ago');
    const text = i18n.t('jobs.boardSummary.health.neverWorkedSince', { since, lng: 'en' });
    expect(text).toBe('has never worked here · first failed 2 days ago');
  });
});
