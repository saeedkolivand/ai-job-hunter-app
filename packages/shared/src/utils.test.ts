import { describe, expect, it } from 'vitest';

import {
  calculateDownloadSpeed,
  calculateTimeRemaining,
  formatBytes,
  formatDownloadSpeed,
  formatTimeRemaining,
  getDeviceTier,
} from './utils';

describe('formatBytes', () => {
  it('formats bytes below 1 KB', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1023)).toBe('1023 B');
  });

  it('formats kilobytes', () => {
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1536)).toBe('1.5 KB');
  });

  it('formats megabytes', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB');
    expect(formatBytes(2.5 * 1024 * 1024)).toBe('2.5 MB');
  });

  it('formats gigabytes', () => {
    expect(formatBytes(1024 * 1024 * 1024)).toBe('1.0 GB');
    expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe('3.0 GB');
  });
});

describe('formatTimeRemaining', () => {
  it('formats seconds', () => {
    expect(formatTimeRemaining(0)).toBe('0s');
    expect(formatTimeRemaining(45)).toBe('45s');
    expect(formatTimeRemaining(59.4)).toBe('59s');
  });

  it('formats minutes', () => {
    expect(formatTimeRemaining(60)).toBe('1m');
    expect(formatTimeRemaining(125)).toBe('2m');
    expect(formatTimeRemaining(3599)).toBe('60m');
  });

  it('formats hours', () => {
    expect(formatTimeRemaining(3600)).toBe('1h');
    expect(formatTimeRemaining(7200)).toBe('2h');
  });
});

describe('getDeviceTier', () => {
  it('classifies high-end by RAM alone', () => {
    expect(getDeviceTier(16)).toEqual({ label: 'High-end', color: 'text-emerald-400' });
    expect(getDeviceTier(32, 16)).toEqual({ label: 'High-end', color: 'text-emerald-400' });
  });

  it('classifies high-end by 8GB RAM + 8 cores', () => {
    expect(getDeviceTier(8, 8).label).toBe('High-end');
  });

  it('classifies mid-range', () => {
    expect(getDeviceTier(8).label).toBe('Mid-range');
    expect(getDeviceTier(4, 4).label).toBe('Mid-range');
  });

  it('classifies low-end', () => {
    expect(getDeviceTier(4).label).toBe('Low-end');
    expect(getDeviceTier(2, 2)).toEqual({ label: 'Low-end', color: 'text-amber-400' });
  });

  it('treats missing cpuCount as zero cores', () => {
    expect(getDeviceTier(8).label).toBe('Mid-range');
    expect(getDeviceTier(4).label).toBe('Low-end');
  });
});

describe('calculateDownloadSpeed', () => {
  it('returns bytes per second over an interval', () => {
    // 1 MB downloaded over 1 second.
    expect(calculateDownloadSpeed(1024 * 1024, 0, 1000, 0)).toBeCloseTo(1024 * 1024);
  });

  it('returns 0 when the interval is too small', () => {
    expect(calculateDownloadSpeed(2000, 1000, 1040, 1000)).toBe(0);
  });

  it('returns 0 when no bytes were added', () => {
    expect(calculateDownloadSpeed(1000, 1000, 2000, 1000)).toBe(0);
    expect(calculateDownloadSpeed(500, 1000, 2000, 1000)).toBe(0);
  });
});

describe('formatDownloadSpeed', () => {
  it('returns empty string for non-positive or non-finite input', () => {
    expect(formatDownloadSpeed(0)).toBe('');
    expect(formatDownloadSpeed(-5)).toBe('');
    expect(formatDownloadSpeed(Infinity)).toBe('');
    expect(formatDownloadSpeed(NaN)).toBe('');
  });

  it('formats KB/s', () => {
    expect(formatDownloadSpeed(1024)).toBe('1.0 KB/s');
  });

  it('formats MB/s', () => {
    expect(formatDownloadSpeed(1024 * 1024)).toBe('1.0 MB/s');
    expect(formatDownloadSpeed(5 * 1024 * 1024)).toBe('5.0 MB/s');
  });
});

describe('calculateTimeRemaining', () => {
  it('returns seconds remaining at the given rate', () => {
    expect(calculateTimeRemaining(1000, 0, 100)).toBe(10);
    expect(calculateTimeRemaining(1000, 500, 100)).toBe(5);
  });

  it('returns 0 for invalid rate or sizes', () => {
    expect(calculateTimeRemaining(1000, 0, 0)).toBe(0);
    expect(calculateTimeRemaining(1000, 0, Infinity)).toBe(0);
    expect(calculateTimeRemaining(0, 0, 100)).toBe(0);
  });

  it('returns 0 once fully downloaded', () => {
    expect(calculateTimeRemaining(1000, 1000, 100)).toBe(0);
    expect(calculateTimeRemaining(1000, 1200, 100)).toBe(0);
  });
});
