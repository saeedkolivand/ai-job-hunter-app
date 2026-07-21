import { describe, expect, it } from 'vitest';

import type { JobRecord } from '@/features/monitoring/types';

import { binLast24Hours, LAST_24H_BUCKETS } from './last-24-hours';

const HOUR = 60 * 60 * 1000;
/** 2026-01-15T12:00:00Z — a fixed "now" so the buckets are deterministic. */
const NOW = Date.UTC(2026, 0, 15, 12, 0, 0);

function job(overrides: Partial<JobRecord> & { finishedAt: number }): JobRecord {
  return {
    id: `job-${overrides.finishedAt}`,
    kind: 'scrape.run',
    status: 'completed',
    progress: 1,
    createdAt: overrides.finishedAt,
    updatedAt: overrides.finishedAt,
    ...overrides,
  };
}

describe('binLast24Hours', () => {
  it('returns 24 empty buckets for no jobs', () => {
    expect(binLast24Hours([], NOW)).toEqual(Array.from({ length: LAST_24H_BUCKETS }, () => 0));
  });

  it('excludes completions older than the window', () => {
    // Same hour-of-day as `now`, but three days ago. Binning by
    // `new Date(ts).getHours()` put this in "today's 12:00" bar.
    const bins = binLast24Hours([job({ finishedAt: NOW - 72 * HOUR })], NOW);
    expect(bins.reduce((a, b) => a + b, 0)).toBe(0);
  });

  it('bins in-window completions oldest-first', () => {
    const bins = binLast24Hours(
      [
        job({ finishedAt: NOW - 30 * 60 * 1000 }), // half an hour ago → last bucket
        job({ finishedAt: NOW - 23 * HOUR }), // nearly a day ago → first bucket
      ],
      NOW
    );
    expect(bins[LAST_24H_BUCKETS - 1]).toBe(1);
    expect(bins[0]).toBe(1);
    expect(bins.reduce((a, b) => a + b, 0)).toBe(2);
  });

  it('counts only completed jobs', () => {
    const ts = NOW - HOUR;
    const bins = binLast24Hours(
      [
        job({ finishedAt: ts, status: 'failed' }),
        job({ finishedAt: ts, status: 'cancelled' }),
        job({ finishedAt: ts, status: 'running' }),
        job({ finishedAt: ts }),
      ],
      NOW
    );
    expect(bins.reduce((a, b) => a + b, 0)).toBe(1);
  });

  it('falls back to updatedAt when finishedAt is absent', () => {
    const ts = NOW - 2 * HOUR;
    const record: JobRecord = {
      id: 'job-no-finished-at',
      kind: 'ai.generate',
      status: 'completed',
      progress: 1,
      createdAt: ts,
      updatedAt: ts,
    };
    expect(binLast24Hours([record], NOW).reduce((a, b) => a + b, 0)).toBe(1);
  });

  it('ignores a timestamp in the future', () => {
    const bins = binLast24Hours([job({ finishedAt: NOW + HOUR })], NOW);
    expect(bins.reduce((a, b) => a + b, 0)).toBe(0);
  });
});
