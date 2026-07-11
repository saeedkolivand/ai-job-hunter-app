/**
 * mergePostings — id-merge + cross-source canonical-key dedup (trust PR E, stage 3).
 *
 * Pass 1 (backend-wins-by-id) is also covered by the pure-function block in
 * JobsPage.dedup-throttle.test.tsx; this file focuses on pass 2 — collapsing the
 * same job surfaced under DIFFERENT ids by two sources, and the survivor policy
 * (keep-incumbent identity, union interactions, byte-length description upgrade,
 * fill-only-missing enrichment).
 */
import { describe, expect, it } from 'vitest';

import type { JobInteraction, JobTrustAssessment } from '@ajh/shared';

import type { Posting } from '../types';
import { mergePostings } from './merge-postings';

function makePosting(overrides: Partial<Posting> & { id: string }): Posting {
  return {
    source: 'linkedin',
    externalId: overrides.id,
    url: `https://example.com/${overrides.id}`,
    title: 'Engineer',
    company: 'Acme',
    description: '',
    capturedAt: 0,
    ...overrides,
  };
}

describe('mergePostings — cross-source canonical-key dedup', () => {
  it('two streamed rows for the same job (different ids) collapse to one, first-seen kept', () => {
    // The live-stream window: the manual-scrape stream is NOT deduped by the
    // engine, so two boards emit the same job under different ids until this pass.
    const gh = makePosting({ id: 'gh-1', url: 'https://acme.example/jobs/42' });
    const li = makePosting({ id: 'li-9', url: 'https://www.acme.example/jobs/42?utm_source=x' });
    const result = mergePostings([], [gh, li]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('gh-1');
  });

  it('the richer (longest) description survives, incumbent id preserved', () => {
    const incumbent = makePosting({
      id: 'a',
      url: 'https://acme.example/jobs/42',
      description: 'short snippet',
    });
    const richer = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      description: 'a much longer, full job description with the entire body text',
    });
    const result = mergePostings([], [incumbent, richer]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('a');
    expect(result[0]?.description).toBe(richer.description);
  });

  it('a persisted row with user-state survives a streamed duplicate: id + interactions kept, description upgraded', () => {
    const bookmark: JobInteraction = {
      jobId: 'backend-1',
      title: 'Engineer',
      company: 'Acme',
      url: 'https://acme.example/jobs/42',
      source: 'linkedin',
      interactionType: 'bookmarked',
      timestamp: 1,
    };
    const backend = makePosting({
      id: 'backend-1',
      url: 'https://acme.example/jobs/42',
      description: 'persisted text',
      interactions: [bookmark],
    });
    const live = makePosting({
      id: 'live-9',
      url: 'https://www.acme.example/jobs/42#apply',
      description: 'a much longer streamed description with the full body',
    });
    const result = mergePostings([backend], [live]);
    expect(result).toHaveLength(1);
    // Survivor keeps the incumbent's id (selection keys on id); `live` here has no
    // interactions to union in, so the incumbent's bookmark alone survives.
    expect(result[0]?.id).toBe('backend-1');
    expect(result[0]?.interactions).toEqual([bookmark]);
    // ...but upgrades to the fuller description.
    expect(result[0]?.description).toBe(live.description);
  });

  it('interactions from both incumbent and challenger are UNIONED, not dropped', () => {
    // Two already-persisted legacy duplicates (pre-dating the engine's dedup pass)
    // can each carry distinct saved/applied state — collapsing must not silently
    // drop the challenger's.
    const saved: JobInteraction = {
      jobId: 'a',
      title: 'Engineer',
      company: 'Acme',
      url: 'https://acme.example/jobs/42',
      source: 'linkedin',
      interactionType: 'bookmarked',
      timestamp: 1,
    };
    const applied: JobInteraction = {
      jobId: 'b',
      title: 'Engineer',
      company: 'Acme',
      url: 'https://acme.example/jobs/42',
      source: 'indeed',
      interactionType: 'applied',
      timestamp: 2,
    };
    const incumbent = makePosting({
      id: 'a',
      url: 'https://acme.example/jobs/42',
      interactions: [saved],
    });
    const challenger = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      interactions: [applied],
    });
    const result = mergePostings([], [incumbent, challenger]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('a');
    expect(result[0]?.interactions).toEqual([saved, applied]);
  });

  it('exact-duplicate interaction entries collapse to one on union', () => {
    const dup: JobInteraction = {
      jobId: 'a',
      title: 'Engineer',
      company: 'Acme',
      url: 'https://acme.example/jobs/42',
      source: 'linkedin',
      interactionType: 'viewed',
      timestamp: 5,
    };
    const incumbent = makePosting({
      id: 'a',
      url: 'https://acme.example/jobs/42',
      interactions: [dup],
    });
    const challenger = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      interactions: [{ ...dup }],
    });
    const result = mergePostings([], [incumbent, challenger]);
    expect(result[0]?.interactions).toHaveLength(1);
  });

  it('url-less rows with the same title+company collapse via the fallback key', () => {
    const a = makePosting({
      id: 'a',
      url: '',
      title: 'Rust Engineer',
      company: 'Acme',
      description: 'x',
    });
    const b = makePosting({
      id: 'b',
      url: '   ',
      title: 'rust engineer',
      company: '  ACME ',
      description: 'a longer description',
    });
    const result = mergePostings([], [a, b]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('a');
    expect(result[0]?.description).toBe('a longer description');
  });

  it('description upgrade compares UTF-8 BYTE length, not UTF-16 code units (matches Rust str::len)', () => {
    // Incumbent: 5 ASCII chars = 5 code units = 5 bytes.
    // Challenger: 4 "€" (U+20AC) = 4 code units (fewer than incumbent) but 12 UTF-8
    // bytes (more than incumbent) — a code-unit compare would WRONGLY keep the
    // incumbent; the byte-length compare correctly upgrades to the challenger,
    // matching what Rust's byte-based `desc_len`/comparison would pick.
    const incumbent = makePosting({
      id: 'a',
      url: 'https://acme.example/jobs/42',
      description: '12345',
    });
    const challenger = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      description: '€€€€',
    });
    expect(challenger.description.length).toBeLessThan(incumbent.description.length);
    const result = mergePostings([], [incumbent, challenger]);
    expect(result[0]?.id).toBe('a');
    expect(result[0]?.description).toBe('€€€€');
  });

  it('trust is filled from the challenger when the incumbent lacks it', () => {
    const trust: JobTrustAssessment = { score: 80, level: 'high', flags: [] };
    const incumbent = makePosting({ id: 'a', url: 'https://acme.example/jobs/42' });
    const challenger = makePosting({ id: 'b', url: 'https://acme.example/jobs/42', trust });
    const result = mergePostings([], [incumbent, challenger]);
    expect(result[0]?.id).toBe('a');
    expect(result[0]?.trust).toEqual(trust);
  });

  it("the incumbent's own trust is never overwritten by the challenger", () => {
    const incumbentTrust: JobTrustAssessment = { score: 90, level: 'high', flags: [] };
    const challengerTrust: JobTrustAssessment = {
      score: 20,
      level: 'low',
      flags: ['suspiciousDomain'],
    };
    const incumbent = makePosting({
      id: 'a',
      url: 'https://acme.example/jobs/42',
      trust: incumbentTrust,
    });
    const challenger = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      trust: challengerTrust,
    });
    const result = mergePostings([], [incumbent, challenger]);
    expect(result[0]?.trust).toEqual(incumbentTrust);
  });

  it('every enrichment field the challenger carries survives a collapse onto a bare incumbent (fill-list guard)', () => {
    // Mechanical pin on collapseDuplicate's fill-list: builds a challenger with
    // every enrichment key `JobPosting.extra` carries today (salaryMin/Max/
    // Currency, remote, trust) set, and a bare incumbent with none of them, then
    // asserts ALL of them survive the collapse. A future `extra` key added to the
    // Rust side without a matching entry in collapseDuplicate's fill-list makes
    // THIS test fail once the new field is added to the fixture below — when you
    // add a key here, add the matching `?? ` line in collapseDuplicate too.
    const trust: JobTrustAssessment = { score: 55, level: 'medium', flags: [] };
    const incumbent = makePosting({ id: 'a', url: 'https://acme.example/jobs/42' });
    const challenger = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      remote: true,
      salaryMin: 80000,
      salaryMax: 100000,
      salaryCurrency: 'GBP',
      trust,
    });
    const result = mergePostings([], [incumbent, challenger]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('a'); // incumbent identity kept
    expect(result[0]?.remote).toBe(true);
    expect(result[0]?.salaryMin).toBe(80000);
    expect(result[0]?.salaryMax).toBe(100000);
    expect(result[0]?.salaryCurrency).toBe('GBP');
    expect(result[0]?.trust).toEqual(trust);
  });

  it('equal-length descriptions tie to the incumbent (no upgrade)', () => {
    const a = makePosting({ id: 'a', url: 'https://acme.example/jobs/42', description: 'abcd' });
    const b = makePosting({ id: 'b', url: 'https://acme.example/jobs/42', description: 'wxyz' });
    const result = mergePostings([], [a, b]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('a');
    expect(result[0]?.description).toBe('abcd');
  });

  it('salary the incumbent LACKS is filled from the challenger (no data dropped on collapse)', () => {
    // The cited harm: a direct-board row (no salary, incumbent) must not erase the
    // aggregator duplicate's salary just because it won on description length.
    const directBoard = makePosting({
      id: 'gh-1',
      url: 'https://acme.example/jobs/42',
      description: 'a full direct-board description',
    });
    const adzuna = makePosting({
      id: 'adz-9',
      url: 'https://www.acme.example/jobs/42',
      description: 'short',
      salaryMin: 90000,
      salaryMax: 120000,
      salaryCurrency: 'EUR',
    });
    const result = mergePostings([], [directBoard, adzuna]);
    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('gh-1');
    expect(result[0]?.description).toBe('a full direct-board description');
    expect(result[0]?.salaryMin).toBe(90000);
    expect(result[0]?.salaryMax).toBe(120000);
    expect(result[0]?.salaryCurrency).toBe('EUR');
  });

  it("the incumbent's own salary is never overwritten by the challenger", () => {
    const incumbent = makePosting({
      id: 'a',
      url: 'https://acme.example/jobs/42',
      salaryMin: 100000,
      salaryCurrency: 'USD',
    });
    const challenger = makePosting({
      id: 'b',
      url: 'https://acme.example/jobs/42',
      description: 'a longer description that would win the description upgrade',
      salaryMin: 50000,
      salaryCurrency: 'EUR',
    });
    const result = mergePostings([], [incumbent, challenger]);
    expect(result).toHaveLength(1);
    expect(result[0]?.salaryMin).toBe(100000);
    expect(result[0]?.salaryCurrency).toBe('USD');
    // Description still upgrades even while salary is preserved.
    expect(result[0]?.description).toBe(challenger.description);
  });

  it('distinct jobs are untouched and keep input order', () => {
    const a = makePosting({ id: 'a', url: 'https://acme.example/jobs/1' });
    const b = makePosting({ id: 'b', url: 'https://acme.example/jobs/2' });
    const c = makePosting({ id: 'c', url: 'https://acme.example/jobs/3' });
    const result = mergePostings([a, b, c], []);
    expect(result.map((p) => p.id)).toEqual(['a', 'b', 'c']);
  });

  it('still merges by id (backend wins) before the canonical pass', () => {
    const backend = makePosting({ id: 'shared', company: 'BackendCo', url: 'https://x/shared' });
    const live = makePosting({ id: 'shared', company: 'LiveCo', url: 'https://x/shared' });
    const result = mergePostings([backend], [live]);
    expect(result).toHaveLength(1);
    expect(result[0]?.company).toBe('BackendCo');
  });
});

describe('mergePostings — absorbed out-param (selection traceability)', () => {
  it('records the exact root-cause scenario: a selected board-id live row absorbed into the aggregator-id persisted incumbent', () => {
    // boards=[aggregator, board]. `board` streams first (only live copy), the
    // user selects it. The persisted refetch (postings, arriving after
    // job.completed) holds the SAME job under the aggregator's id — the engine's
    // incumbent-selection order follows board input order, not display arrival
    // order — so `aggregator-1` is first-seen (incumbent) and `board-1` is the
    // absorbed challenger.
    const aggregatorPersisted = makePosting({
      id: 'aggregator-1',
      source: 'aggregator',
      url: 'https://acme.example/jobs/42',
    });
    const boardLive = makePosting({
      id: 'board-1',
      source: 'board',
      url: 'https://www.acme.example/jobs/42?utm_source=x',
    });
    const absorbed = new Map<string, string>();
    const result = mergePostings([aggregatorPersisted], [boardLive], absorbed);

    expect(result).toHaveLength(1);
    expect(result[0]?.id).toBe('aggregator-1');
    expect(absorbed.get('board-1')).toBe('aggregator-1');
    expect(absorbed.size).toBe(1);
  });

  it('does not record an entry when no collapse happens (distinct jobs)', () => {
    const a = makePosting({ id: 'a', url: 'https://acme.example/jobs/1' });
    const b = makePosting({ id: 'b', url: 'https://acme.example/jobs/2' });
    const absorbed = new Map<string, string>();
    mergePostings([a], [b], absorbed);
    expect(absorbed.size).toBe(0);
  });

  it('does not record an entry when the id-merge (pass 1) already unified the row (same id both sides)', () => {
    // Pass 1 drops the live copy before pass 2 ever runs — same id, not a
    // cross-source absorb, so nothing should be recorded as "absorbed".
    const backend = makePosting({ id: 'shared', url: 'https://acme.example/jobs/42' });
    const live = makePosting({ id: 'shared', url: 'https://acme.example/jobs/42' });
    const absorbed = new Map<string, string>();
    mergePostings([backend], [live], absorbed);
    expect(absorbed.size).toBe(0);
  });

  it('is a no-op (does not throw) when the caller omits the out-param', () => {
    const a = makePosting({ id: 'a', url: 'https://acme.example/jobs/42' });
    const b = makePosting({ id: 'b', url: 'https://acme.example/jobs/42' });
    expect(() => mergePostings([], [a, b])).not.toThrow();
  });
});
