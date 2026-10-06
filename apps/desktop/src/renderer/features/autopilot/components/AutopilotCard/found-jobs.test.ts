/**
 * sortFoundJobsByDate — pure comparator: banding, tiebreak, non-mutation
 *
 * Shared mocks + fixtures live in ./test-render.
 */

import { describe, expect, it } from 'vitest';

import type { AutopilotFoundJob } from '@ajh/shared';

import { sortFoundJobsByDate } from './found-jobs';

describe('sortFoundJobsByDate', () => {
  const dated = (url: string, postedAt: number): AutopilotFoundJob => ({
    title: url,
    company: 'Acme',
    url,
    foundAt: 0,
    postedAt,
  });
  const undated = (url: string): AutopilotFoundJob => ({
    title: url,
    company: 'Acme',
    url,
    foundAt: 0,
  });

  it('bands dated jobs before undated jobs regardless of input order (sortBy="newest")', () => {
    const input = [undated('u1'), dated('d1', 1000), undated('u2'), dated('d2', 2000)];
    const result = sortFoundJobsByDate(input, 'newest');
    expect(result.map((j) => j.url)).toEqual(['d2', 'd1', 'u1', 'u2']);
  });

  // A `postedAt ?? 0` fallback (instead of a real dated/undated branch) would
  // pass the "newest" banding case above by accident — timestamp 0 sorts last
  // in a descending comparator anyway — but breaks exactly here: ascending
  // "oldest" would sort the undated 0-fallback rows to the FRONT, not the
  // trailing band. This is the case that actually needs the explicit banding.
  it('bands dated jobs before undated jobs regardless of input order (sortBy="oldest")', () => {
    const input = [undated('u1'), dated('d1', 1000), undated('u2'), dated('d2', 2000)];
    const result = sortFoundJobsByDate(input, 'oldest');
    expect(result.map((j) => j.url)).toEqual(['d1', 'd2', 'u1', 'u2']);
  });

  // Pinpoint case for the CodeRabbit round-1 finding: `postedAt: 0` (epoch)
  // must band as DATED, matching the render guard's `typeof === 'number'`
  // contract — a falsy-0 check anywhere in this pipeline would sink it into
  // the undated/trailing band instead.
  it("treats postedAt: 0 as dated, not undated (shares the render guard's typeof contract)", () => {
    const input = [undated('u1'), dated('epoch', 0)];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['epoch', 'u1']);
  });

  it('orders the dated band newest-first for sortBy="newest"', () => {
    const input = [dated('a', 1000), dated('b', 3000), dated('c', 2000)];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['b', 'c', 'a']);
  });

  it('orders the dated band oldest-first for sortBy="oldest"', () => {
    const input = [dated('a', 1000), dated('b', 3000), dated('c', 2000)];
    expect(sortFoundJobsByDate(input, 'oldest').map((j) => j.url)).toEqual(['a', 'c', 'b']);
  });

  it('tiebreaks equal postedAt values by url, for a deterministic order across renders', () => {
    const input = [dated('zzz', 1000), dated('aaa', 1000)];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['aaa', 'zzz']);
  });

  it('tiebreaks two undated jobs by url', () => {
    const input = [undated('zzz'), undated('aaa')];
    expect(sortFoundJobsByDate(input, 'newest').map((j) => j.url)).toEqual(['aaa', 'zzz']);
  });

  it('does NOT mutate the input array (ADR-020: the persisted order feeds AI-note recipient selection)', () => {
    const input = [dated('b', 3000), dated('a', 1000), undated('c')];
    const originalOrder = input.map((j) => j.url);

    const result = sortFoundJobsByDate(input, 'newest');

    expect(input.map((j) => j.url)).toEqual(originalOrder); // input order untouched
    expect(result).not.toBe(input); // a fresh array was returned, not the input reordered in place
  });
});
