import { vi } from 'vitest';
import { act } from '@testing-library/react';

import type { JobEvent, PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';

import { bus, fetchJobMock, startMock } from './test-mocks';

export const RUN_ID = 'run-1';
export const JOB_ID = 'job-1';

export function stage(
  name: string,
  phase: PipelineStageEvent['phase'],
  index: number,
  runId = RUN_ID
): PipelineStageEvent {
  return { runId, jobId: JOB_ID, stage: name, phase, index, total: 6, attempt: 1 };
}

export function detail(
  status: PipelineRunDetail['status'],
  stoppedReason?: string | null
): PipelineRunDetail {
  return {
    runId: RUN_ID,
    jobUrl: 'https://example.test/job',
    kind: 'resume',
    depth: 'quality',
    status,
    startedAt: 1,
    ...(stoppedReason !== undefined ? { stoppedReason } : {}),
    metrics: {},
    events: [],
    report: null,
    resumeText: 'final document',
  };
}

export function jobFailed(jobId: string, data?: unknown): JobEvent {
  return { type: 'job.failed', jobId, ...(data !== undefined ? { data } : {}), ts: 1 };
}

/** Start a run and drive the stage stream through the whole pipeline. */
export const STAGES = [
  'analyze_job',
  'match_evidence',
  'strategy',
  'draft',
  'cover_letter',
  'validate',
  'repair',
  'humanize',
];

export const START_ARGS = {
  resumeId: 'doc-1',
  jobId: 'posting-1',
  jobUrl: '',
  targetLanguage: 'en',
  topRequirements: [],
  coverLetterText: '',
  includeCoverLetter: false,
};

/** Start a run with {@link START_ARGS} (plus `over`) inside `act`. */
export const startRun = (
  result: { current: { start: (a: typeof START_ARGS) => Promise<unknown> } },
  over: Partial<typeof START_ARGS> = {}
) =>
  act(async () => {
    await result.current.start({ ...START_ARGS, ...over });
  });

/** Reset the bus + mocks to a clean baseline (call in `beforeEach`). */
export function resetPipelineMocks() {
  vi.clearAllMocks();
  bus.stage = null;
  bus.delta = null;
  bus.letterDelta = null;
  bus.thinking = null;
  bus.job = null;
  bus.detail = null;
  bus.live = false;
  bus.recordError = null;
  startMock.mutateAsync.mockResolvedValue({ runId: RUN_ID, jobId: JOB_ID });
  // Default: no umbrella-job failure raced the start — matches every test
  // that isn't specifically about that race.
  fetchJobMock.mockResolvedValue(null);
}
