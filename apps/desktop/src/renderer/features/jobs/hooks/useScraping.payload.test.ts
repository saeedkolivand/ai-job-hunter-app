/**
 * useScraping — the scrapeBoards payload and the replace-vs-append search
 * signature.
 *
 * Exercises the actual hook (not a local clone) to verify that `companies` /
 * `workTypes` are conditionally included in the payload so the IPC contract is
 * honoured and the Rust engine's is_empty() skip check behaves correctly, and
 * that the signature decides whether "Show more" replaces or appends.
 */
import { beforeEach, describe, expect, it } from 'vitest';

import { useSessionStore } from '@/store/session-store';

import {
  makeForm,
  mountAndStart,
  mountForm,
  mutateAsync,
  resetScraping,
  sentPayload,
  sentReplace,
  start,
} from './scraping-harness';

beforeEach(resetScraping);

describe('useScraping — optional array fields in scrapeBoards payload', () => {
  it.each([['companies'], ['workTypes']] as const)(
    'omits %s from the payload when the array is empty',
    async (field) => {
      await mountAndStart(makeForm({ [field]: [] }));

      expect(mutateAsync).toHaveBeenCalledOnce();
      expect(sentPayload(0)).not.toHaveProperty(field);
    }
  );

  it.each([
    ['companies', ['stripe', 'airbnb']],
    ['workTypes', ['remote', 'hybrid']],
  ] as const)('includes %s in the payload when the array is non-empty', async (field, value) => {
    await mountAndStart(makeForm({ [field]: value }));

    expect(mutateAsync).toHaveBeenCalledOnce();
    expect(sentPayload(0)).toHaveProperty(field, value);
  });
});

describe('useScraping — workTypes in the replace-vs-append search signature', () => {
  it('replaces (not appends) when only the requested work type differs', async () => {
    // The bug this pins: omitting workTypes from the signature means changing
    // the work-type filter and pressing "Show more" would APPEND results from
    // a different filter instead of replacing them.
    const view = mountForm(makeForm({ workTypes: ['remote'] }));
    await start(view);

    view.rerender({ form: makeForm({ workTypes: ['hybrid'] }) });
    await start(view);

    expect(sentReplace(1)).toBe(true);
  });

  it('appends when the requested work-type set is identical regardless of order', async () => {
    const view = mountForm(makeForm({ workTypes: ['remote', 'hybrid'] }));
    await start(view);

    // Sorted+joined signature: order must not matter.
    view.rerender({ form: makeForm({ workTypes: ['hybrid', 'remote'] }) });
    await start(view);

    expect(sentReplace(0)).toBe(true);
    expect(sentReplace(1)).toBeUndefined();
  });
});

describe('useScraping — seeding the command-bar view filter (jobs.workTypes)', () => {
  const viewWorkTypes = () => useSessionStore.getState().jobs.workTypes;

  it('seeds jobs.workTypes from the scrape-time selection on a NEW search', async () => {
    await mountAndStart(makeForm({ workTypes: ['remote'] }));

    expect(viewWorkTypes()).toEqual(['remote']);
  });

  it('resets jobs.workTypes to empty on a NEW search with no work-type selection', async () => {
    // A stale selection from a PREVIOUS search (or a manual view-filter
    // toggle) must not survive an unrelated new search.
    useSessionStore.setState((s) => ({ jobs: { ...s.jobs, workTypes: ['hybrid'] } }));
    await mountAndStart(makeForm({ query: 'engineer', workTypes: [] }));

    expect(viewWorkTypes()).toEqual([]);
  });

  it('does NOT stomp a mid-session view-filter widening on "Show more" (same search)', async () => {
    const view = await mountAndStart(makeForm({ workTypes: ['remote'] }));
    expect(viewWorkTypes()).toEqual(['remote']);

    // User manually widens the view-only filter after the search lands.
    useSessionStore.setState((s) => ({ jobs: { ...s.jobs, workTypes: ['remote', 'hybrid'] } }));

    // "Show more": identical scrapeForm, so the signature is unchanged and
    // this call appends rather than replacing.
    await start(view, 50);

    expect(viewWorkTypes()).toEqual(['remote', 'hybrid']);
  });
});

describe('useScraping — geo fields in the replace-vs-append signature', () => {
  it('replaces (not appends) when only the countryCode differs', async () => {
    const view = mountForm(makeForm({ countryCode: 'US' }));

    // First run seeds the last-search signature.
    await start(view);

    // Same keywords, different country → a different market must REPLACE the
    // stale results. This fails if countryCode is missing from the signature.
    view.rerender({ form: makeForm({ countryCode: 'DE' }) });
    await start(view);

    expect(sentReplace(1)).toBe(true);
  });

  it('replaces (not appends) when only the search radius differs', async () => {
    const view = mountForm(makeForm({ radiusKm: 0 }));
    await start(view);

    // Same city, wider radius → a different search area must REPLACE. This
    // fails if radiusKm is missing from the signature.
    view.rerender({ form: makeForm({ radiusKm: 25 }) });
    await start(view);

    expect(sentReplace(1)).toBe(true);
  });

  it('appends (does not replace) when the search is byte-for-byte identical', async () => {
    const view = mountForm(makeForm({ countryCode: 'US' }));
    await start(view);

    // Identical form (including geo) → "show more" semantics: keep + append.
    view.rerender({ form: makeForm({ countryCode: 'US' }) });
    await start(view);

    // The first run REPLACES (nothing on screen belongs to it yet), the second
    // APPENDS — `replace` is omitted entirely from the payload.
    expect(sentReplace(0)).toBe(true);
    expect(sentReplace(1)).toBeUndefined();
  });
});
