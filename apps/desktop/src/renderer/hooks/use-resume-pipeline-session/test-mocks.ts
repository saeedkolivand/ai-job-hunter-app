/**
 * Shared mock state + factories for the useResumePipelineSession tests. The hook's
 * whole job is to combine three sources (stage events, the draft stream, the run
 * record) into one state, so the tests drive all three by hand through `bus`
 * rather than through a mock IPC client. `vi.mock` is hoisted per test file, so
 * each file wires these in with
 * `vi.mock('@/services/use-jobs', async () => (await import('./test-mocks')).jobsMock())`.
 */
import { type Mock, vi } from 'vitest';

import type { JobEvent, PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

export const startMock: { mutateAsync: Mock; isPending: boolean } = {
  mutateAsync: vi.fn(),
  isPending: false,
};
export const cancelJobMock: { mutate: Mock } = { mutate: vi.fn() };
export const refreshRunsMock: Mock = vi.fn();
export const fetchJobMock: Mock = vi.fn();
export const bus = {
  stage: null as ((e: PipelineStageEvent) => void) | null,
  delta: null as ((d: string) => void) | null,
  /** The letter's own stream (`<jobId>#letter`). */
  letterDelta: null as ((d: string) => void) | null,
  thinking: null as ((d: string) => void) | null,
  job: null as ((e: JobEvent) => void) | null,
  detail: null as PipelineRunDetail | null,
  live: false,
  recordError: null as Error | null,
};

export const pipelineMock = (): Record<string, unknown> => ({
  useStartResumePipelineRun: () => startMock,
  usePipelineRun: (_runId: string | null, live: boolean) => {
    bus.live = live;
    return { data: bus.detail, isError: !!bus.recordError, error: bus.recordError };
  },
  usePipelineStageEvents: (handler?: (e: PipelineStageEvent) => void) => {
    bus.stage = handler ?? null;
  },
  usePipelineDraftStream: (
    jobId: string | null,
    onDelta?: (d: string) => void,
    onThinking?: (d: string) => void
  ) => {
    if (jobId?.endsWith('#letter')) {
      bus.letterDelta = onDelta ?? null;
      return;
    }
    bus.delta = onDelta ?? null;
    bus.thinking = onThinking ?? null;
  },
  useRefreshRunsForJobOnTerminal: refreshRunsMock,
});

export const jobsMock = (): Record<string, unknown> => ({
  useCancelJob: () => cancelJobMock,
  useJobEvents: (handler?: (e: JobEvent) => void) => {
    bus.job = handler ?? null;
  },
  fetchJob: (...args: unknown[]) => fetchJobMock(...args),
});
