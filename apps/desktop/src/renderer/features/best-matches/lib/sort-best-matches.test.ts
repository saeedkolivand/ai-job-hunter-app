import { describe, expect, it } from 'vitest';

import type { AutopilotBestMatch } from '@ajh/shared';

import { sortBestMatches, sortByNewest, sortBySalary } from './sort-best-matches';

function makeMatch(overrides: Partial<AutopilotBestMatch> = {}): AutopilotBestMatch {
  return {
    key: overrides.url ?? 'k',
    title: 'Engineer',
    company: 'Acme',
    url: 'https://example.com/a',
    score: 80,
    scoreSource: 'combined',
    foundAt: 0,
    sources: [],
    ...overrides,
  };
}

describe('sortByNewest', () => {
  it('sorts dated rows postedAt desc', () => {
    const a = makeMatch({ url: 'https://x/a', postedAt: 1_000 });
    const b = makeMatch({ url: 'https://x/b', postedAt: 3_000 });
    const c = makeMatch({ url: 'https://x/c', postedAt: 2_000 });
    expect(sortByNewest([a, b, c]).map((m) => m.url)).toEqual([b.url, c.url, a.url]);
  });

  it('puts the entire dated band ahead of the entire undated band', () => {
    const dated = makeMatch({ url: 'https://x/dated', postedAt: 500 });
    const undated = makeMatch({ url: 'https://x/undated' });
    expect(sortByNewest([undated, dated]).map((m) => m.url)).toEqual([dated.url, undated.url]);
  });

  it('does NOT fall back to foundAt for undated rows — a fresher scrape must not outrank an older one', () => {
    // Both undated; the more recently FOUND one (higher foundAt) must not win —
    // only the url tiebreak decides.
    const staleFind = makeMatch({ url: 'https://x/b', foundAt: 2_000 });
    const earlierFind = makeMatch({ url: 'https://x/a', foundAt: 1_000 });
    expect(sortByNewest([staleFind, earlierFind]).map((m) => m.url)).toEqual([
      earlierFind.url,
      staleFind.url,
    ]);
  });

  it('breaks ties (equal postedAt, or both undated) by url ascending', () => {
    const b = makeMatch({ url: 'https://x/b', postedAt: 1_000 });
    const a = makeMatch({ url: 'https://x/a', postedAt: 1_000 });
    expect(sortByNewest([b, a]).map((m) => m.url)).toEqual([a.url, b.url]);
  });

  it('never mutates the input array', () => {
    const input = [makeMatch({ url: 'https://x/b' }), makeMatch({ url: 'https://x/a' })];
    const snapshot = [...input];
    sortByNewest(input);
    expect(input).toEqual(snapshot);
  });
});

describe('sortBySalary', () => {
  it('sorts by salaryMax desc when present', () => {
    const low = makeMatch({ url: 'https://x/low', salaryMax: 80_000 });
    const high = makeMatch({ url: 'https://x/high', salaryMax: 150_000 });
    expect(sortBySalary([low, high]).map((m) => m.url)).toEqual([high.url, low.url]);
  });

  it('falls back to salaryMin when salaryMax is absent', () => {
    const withMin = makeMatch({ url: 'https://x/min', salaryMin: 90_000 });
    const withMax = makeMatch({ url: 'https://x/max', salaryMax: 60_000 });
    expect(sortBySalary([withMax, withMin]).map((m) => m.url)).toEqual([withMin.url, withMax.url]);
  });

  it('puts every salaried row ahead of every unsalaried row', () => {
    const salaried = makeMatch({ url: 'https://x/salaried', salaryMax: 50_000 });
    const unsalaried = makeMatch({ url: 'https://x/unsalaried' });
    expect(sortBySalary([unsalaried, salaried]).map((m) => m.url)).toEqual([
      salaried.url,
      unsalaried.url,
    ]);
  });

  it('breaks ties by url ascending', () => {
    const b = makeMatch({ url: 'https://x/b', salaryMax: 100_000 });
    const a = makeMatch({ url: 'https://x/a', salaryMax: 100_000 });
    expect(sortBySalary([b, a]).map((m) => m.url)).toEqual([a.url, b.url]);
  });

  it('never mutates the input array', () => {
    const input = [
      makeMatch({ url: 'https://x/b', salaryMax: 1 }),
      makeMatch({ url: 'https://x/a', salaryMax: 2 }),
    ];
    const snapshot = [...input];
    sortBySalary(input);
    expect(input).toEqual(snapshot);
  });
});

describe('sortBestMatches', () => {
  it('"score" returns the backend order untouched (same array reference)', () => {
    const input = [makeMatch({ url: 'https://x/b' }), makeMatch({ url: 'https://x/a' })];
    expect(sortBestMatches(input, 'score')).toBe(input);
  });

  it('"newest" delegates to sortByNewest', () => {
    const a = makeMatch({ url: 'https://x/a', postedAt: 1_000 });
    const b = makeMatch({ url: 'https://x/b', postedAt: 2_000 });
    expect(sortBestMatches([a, b], 'newest')).toEqual(sortByNewest([a, b]));
  });

  it('"salary" delegates to sortBySalary', () => {
    const a = makeMatch({ url: 'https://x/a', salaryMax: 1 });
    const b = makeMatch({ url: 'https://x/b', salaryMax: 2 });
    expect(sortBestMatches([a, b], 'salary')).toEqual(sortBySalary([a, b]));
  });
});
