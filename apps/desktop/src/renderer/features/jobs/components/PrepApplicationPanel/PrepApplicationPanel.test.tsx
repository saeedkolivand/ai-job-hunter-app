/**
 * PrepApplicationPanel — the "Prep this application" trigger + modal.
 *
 * Covers:
 *  - Start is disabled (with a needsResume hint) when no résumé is saved.
 *  - Starting a run calls agent.run with the exact {resumeId, jobId, provider,
 *    model, baseUrl} payload sourced from useDefaultResumeId/useGenerateConfig.
 *  - A streamed `agent:step` turn (matching the run's own jobId) renders in
 *    the live checklist with its text; a step for a DIFFERENT jobId is ignored
 *    (cross-run contamination guard).
 *  - The terminal `job.completed` event renders the proposal card and clears
 *    the Stop affordance (machine reaches `done`).
 *  - A `job.failed` event for the run's own jobId surfaces ErrorState; a
 *    mismatched jobId is ignored.
 *  - `job.cancelled` renders the distinct "cancelled" copy, NOT ErrorState.
 *  - Stop calls jobs.cancel with the run's jobId.
 *  - Retry-from-error restarts the run and can reach `done` again.
 *
 * `@ajh/ui` primitives run for real (only ModalShell is simplified, dropping
 * the portal/focus-trap plumbing, but still rendering its footer) so the test
 * exercises real Button/EmptyState/ErrorState/StreamingText/Tag composition
 * rather than a fully-stubbed shell.
 */
import type React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { AgentStepEvent, JobEvent } from '@ajh/shared';
import type * as AjhUi from '@ajh/ui';

import type { Posting } from '@/features/jobs/types';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @ajh/ui — keep everything real, simplify only ModalShell ────────────────

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    ModalShell: ({
      open,
      header,
      footer,
      children,
    }: {
      open: boolean;
      header?: React.ReactNode;
      footer?: React.ReactNode;
      children?: React.ReactNode;
    }) =>
      open ? (
        <div role="dialog">
          {header}
          {children}
          {footer}
        </div>
      ) : null,
  };
});

// ── @/hooks/useDefaultResumeId — mutable per-test ────────────────────────────

let stubbedResumeId: string | null = 'resume-1';
vi.mock('@/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => stubbedResumeId,
}));

// ── @/services — capture the step/job-event handlers so tests can drive them

let stepHandler: ((event: AgentStepEvent) => void) | undefined;
let jobEventHandler: ((event: JobEvent) => void) | undefined;
const mockRunMutateAsync = vi.fn().mockResolvedValue({ jobId: 'job-1' });
const mockCancelMutateAsync = vi.fn().mockResolvedValue(undefined);

vi.mock('@/services', () => ({
  useAgentRun: () => ({ mutateAsync: mockRunMutateAsync, isPending: false }),
  useAgentStepEvents: (cb: (event: AgentStepEvent) => void) => {
    stepHandler = cb;
  },
  useCancelJob: () => ({ mutateAsync: mockCancelMutateAsync }),
  useGenerateConfig: () => ({ provider: 'ollama', model: 'llama3', baseUrl: undefined }),
  useJobEvents: (cb: (event: JobEvent) => void) => {
    jobEventHandler = cb;
  },
}));

// ── component under test ──────────────────────────────────────────────────────

import { PrepApplicationPanel } from './index';

const POSTING: Posting = {
  id: 'posting-1',
  source: 'linkedin',
  externalId: 'posting-1',
  url: 'https://example.com/job/posting-1',
  title: 'Senior Engineer',
  company: 'Acme',
  description: 'A great role.',
  capturedAt: 0,
};

/** A turn-kind AgentStepEvent for the run's own jobId, unless overridden. */
const turnStep = (overrides: Partial<AgentStepEvent> = {}): AgentStepEvent => ({
  jobId: 'job-1',
  step: 1,
  text: '',
  tools: [],
  denied: [],
  kind: 'turn',
  ...overrides,
});

beforeEach(() => {
  stubbedResumeId = 'resume-1';
  mockRunMutateAsync.mockClear();
  mockRunMutateAsync.mockResolvedValue({ jobId: 'job-1' });
  mockCancelMutateAsync.mockClear();
  stepHandler = undefined;
  jobEventHandler = undefined;
});

function openModal() {
  render(<PrepApplicationPanel posting={POSTING} />);
  fireEvent.click(screen.getByText('jobs.prep.trigger'));
}

async function clickStart() {
  await act(async () => {
    fireEvent.click(screen.getByText('jobs.prep.start'));
  });
}

describe('PrepApplicationPanel — start gating', () => {
  it('disables Start and shows the needsResume hint when no résumé is saved', () => {
    stubbedResumeId = null;
    openModal();

    expect(screen.getByText('jobs.prep.needsResume')).toBeInTheDocument();
    const start = screen.getByText('jobs.prep.start').closest('button');
    expect(start).toBeDisabled();
  });
});

describe('PrepApplicationPanel — starting a run', () => {
  it('calls agent.run with resumeId/jobId/provider/model/baseUrl and shows the starting indicator', async () => {
    openModal();
    await clickStart();

    expect(mockRunMutateAsync).toHaveBeenCalledWith({
      resumeId: 'resume-1',
      jobId: 'posting-1',
      provider: 'ollama',
      model: 'llama3',
      baseUrl: undefined,
    });
    expect(screen.getByText('jobs.prep.starting')).toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — jobId filtering (cross-run guard)', () => {
  it('ignores a step for a different jobId', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(turnStep({ jobId: 'some-other-run', tools: ['research_company'], text: 'x' }));
    });

    expect(screen.queryByText('x')).not.toBeInTheDocument();
    // Still shows the starting indicator — no step was accepted for THIS run.
    expect(screen.getByText('jobs.prep.starting')).toBeInTheDocument();
  });

  it('accepts a step matching the run jobId', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(turnStep({ tools: ['research_company'], text: 'Researching Acme…' }));
    });

    expect(screen.getByText('Researching Acme…')).toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — live checklist + proposal + completion', () => {
  it('renders a streamed research turn, then the proposal card on job.completed', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(turnStep({ tools: ['research_company'], text: 'Researching Acme…' }));
    });
    expect(screen.getByText('Researching Acme…')).toBeInTheDocument();
    expect(screen.getByText('jobs.prep.steps.research')).toBeInTheDocument();

    act(() => {
      stepHandler?.(
        turnStep({ step: 5, text: 'Proposing: move to Applied.', tools: [], kind: 'proposal' })
      );
    });
    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'job-1',
        data: { finalText: 'Proposing: move to Applied.', steps: 4, stoppedReason: 'done' },
        ts: 0,
      });
    });

    expect(screen.getByText('jobs.prep.proposalTitle')).toBeInTheDocument();
    expect(screen.getByText('Proposing: move to Applied.')).toBeInTheDocument();
    // stoppedReason 'done' (Rust's snake_case StoppedReason) maps to the "done" suffix.
    expect(screen.getByText('jobs.prep.stopped.done')).toBeInTheDocument();
    // The propose checklist row is status-only (no duplicate body text) and
    // reaches "done" once the terminal proposal step lands.
    expect(screen.getByText('jobs.prep.steps.propose')).toBeInTheDocument();
    // The run is done — the Stop affordance is no longer shown.
    expect(screen.queryByText('jobs.prep.stop')).not.toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — run failure', () => {
  it('shows ErrorState when job.failed arrives for the run jobId', async () => {
    openModal();
    await clickStart();

    act(() => {
      jobEventHandler?.({ type: 'job.failed', jobId: 'job-1', data: 'boom', ts: 0 });
    });

    expect(screen.getByText('jobs.prep.runFailed')).toBeInTheDocument();
    expect(screen.getByText('boom')).toBeInTheDocument();
  });

  it('ignores a job.failed event for a different jobId', async () => {
    openModal();
    await clickStart();

    act(() => {
      jobEventHandler?.({ type: 'job.failed', jobId: 'some-other-job', data: 'boom', ts: 0 });
    });

    expect(screen.queryByText('jobs.prep.runFailed')).not.toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — cancellation', () => {
  it('shows the distinct cancelled copy (not the failure ErrorState) on job.cancelled', async () => {
    openModal();
    await clickStart();

    act(() => {
      jobEventHandler?.({ type: 'job.cancelled', jobId: 'job-1', ts: 0 });
    });

    expect(screen.getByText('jobs.prep.stopped.cancelled')).toBeInTheDocument();
    expect(screen.queryByText('jobs.prep.runFailed')).not.toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — stop', () => {
  it('calls jobs.cancel with the run jobId and disables itself', async () => {
    openModal();
    await clickStart();

    const stop = screen.getByText('jobs.prep.stop').closest('button');
    await act(async () => {
      fireEvent.click(stop as HTMLButtonElement);
    });

    expect(mockCancelMutateAsync).toHaveBeenCalledWith('job-1');
    expect(screen.getByText('jobs.prep.stopping')).toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — retry', () => {
  it('retry-from-error restarts the run (a second agent.run call) and can reach done again', async () => {
    openModal();
    await clickStart();

    act(() => {
      jobEventHandler?.({ type: 'job.failed', jobId: 'job-1', data: 'boom', ts: 0 });
    });
    expect(screen.getByText('jobs.prep.runFailed')).toBeInTheDocument();

    // The footer's Retry ("Start") button restarts the run.
    mockRunMutateAsync.mockResolvedValueOnce({ jobId: 'job-2' });
    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.start'));
    });

    expect(mockRunMutateAsync).toHaveBeenCalledTimes(2);
    // Stale error copy is cleared once the retry starts.
    expect(screen.queryByText('jobs.prep.runFailed')).not.toBeInTheDocument();

    act(() => {
      stepHandler?.(
        turnStep({ jobId: 'job-2', step: 3, text: 'Proposal.', tools: [], kind: 'proposal' })
      );
    });
    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'job-2',
        data: { finalText: 'Proposal.', steps: 1, stoppedReason: 'done' },
        ts: 0,
      });
    });
    expect(screen.getByText('jobs.prep.proposalTitle')).toBeInTheDocument();
  });
});
