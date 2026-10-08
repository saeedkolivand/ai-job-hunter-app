import { describe, expect, it } from 'vitest';
import { renderHook } from '@testing-library/react';

import type { JobRecord } from '@/features/monitoring/types';

import { useJobMetrics } from './useJobMetrics';

const job = (status: string) => ({ id: status, status }) as unknown as JobRecord;

describe('useJobMetrics', () => {
  it('counts cancelled separately and keeps it out of the success rate', () => {
    const jobs = [job('completed'), job('completed'), job('cancelled'), job('cancelled')];
    const { result } = renderHook(() => useJobMetrics(jobs));

    expect(result.current.counters).toMatchObject({ completed: 2, failed: 0, cancelled: 2 });
    expect(result.current.successRate).toBe(100);
  });

  it('still counts failed against the success rate', () => {
    const jobs = [job('completed'), job('failed')];
    const { result } = renderHook(() => useJobMetrics(jobs));

    expect(result.current.successRate).toBe(50);
  });
});
