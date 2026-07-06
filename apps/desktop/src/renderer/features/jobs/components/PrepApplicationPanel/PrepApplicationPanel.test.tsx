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

import type { AgentStepEvent, JobEvent, JobRecord } from '@ajh/shared';
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
// Drives the `useJob` reconciliation fallback — undefined unless a test
// stubs it to simulate a fast-fail job the event path never delivered.
let stubbedJobRecord: JobRecord | undefined;
const mockRunMutateAsync = vi.fn().mockResolvedValue({ jobId: 'job-1' });
const mockCancelMutateAsync = vi.fn().mockResolvedValue(undefined);
const mockConfirmMutateAsync = vi.fn().mockResolvedValue({ ok: true });

vi.mock('@/services', () => ({
  useAgentRun: () => ({ mutateAsync: mockRunMutateAsync, isPending: false }),
  useAgentConfirm: () => ({ mutateAsync: mockConfirmMutateAsync, isPending: false }),
  useAgentStepEvents: (cb: (event: AgentStepEvent) => void) => {
    stepHandler = cb;
  },
  useCancelJob: () => ({ mutateAsync: mockCancelMutateAsync }),
  useGenerateConfig: () => ({ provider: 'ollama', model: 'llama3', baseUrl: undefined }),
  useJob: () => ({ data: stubbedJobRecord }),
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

/** A `confirm_request`-kind AgentStepEvent for the run's own jobId. */
const confirmRequestStep = (overrides: Partial<AgentStepEvent> = {}): AgentStepEvent => ({
  jobId: 'job-1',
  step: 3,
  text: '',
  tools: ['save_cover_letter'],
  denied: [],
  kind: 'confirm_request',
  confirm: {
    callId: '3-save_cover_letter',
    tool: 'save_cover_letter',
    args: { coverLetterText: 'Draft.' },
  },
  ...overrides,
});

beforeEach(() => {
  stubbedResumeId = 'resume-1';
  mockRunMutateAsync.mockClear();
  mockRunMutateAsync.mockResolvedValue({ jobId: 'job-1' });
  mockCancelMutateAsync.mockClear();
  mockConfirmMutateAsync.mockClear();
  mockConfirmMutateAsync.mockResolvedValue({ ok: true });
  stepHandler = undefined;
  jobEventHandler = undefined;
  stubbedJobRecord = undefined;
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

describe('PrepApplicationPanel — useJob reconciliation fallback (no stuck spinner)', () => {
  it('reaches `error` via useJob reconciliation when the job already failed before any jobs:event arrived', async () => {
    // Simulates the race: a fast backend validation failure emits its
    // terminal `jobs:event` before `agent.run`'s IPC round-trip resolves, so
    // `handleJobEvent`'s `runJobId` guard never sees it. `jobEventHandler` is
    // intentionally never invoked here — the fallback must reconcile solely
    // from the job's already-failed status once `runJobId` is known.
    stubbedJobRecord = {
      id: 'job-1',
      kind: 'ai.generate',
      status: 'failed',
      progress: 0,
      payload: {},
      error: 'Provider does not support tool calls.',
      retries: 0,
      maxRetries: 0,
      createdAt: 0,
      updatedAt: 0,
    };
    openModal();
    await clickStart();

    expect(screen.getByText('jobs.prep.runFailed')).toBeInTheDocument();
    expect(screen.getByText('Provider does not support tool calls.')).toBeInTheDocument();
    // Would still show the busy spinner if the machine were stuck in `planning`.
    expect(screen.queryByText('jobs.prep.starting')).not.toBeInTheDocument();
  });

  it('reaches `done` via useJob reconciliation and renders the proposal when job.completed never arrived (agent:step already streamed it)', async () => {
    openModal();
    await clickStart();

    // The `agent:step` channel is unaffected by the `jobs:event` race — the
    // proposal narration already streamed in. `job.result` (read here) is a
    // DIFFERENT field than the live event path's `event.data` — this is the
    // regression check for that mapping.
    stubbedJobRecord = {
      id: 'job-1',
      kind: 'ai.generate',
      status: 'completed',
      progress: 100,
      payload: {},
      result: { finalText: 'Proposal draft text.', steps: 5, stoppedReason: 'done' },
      retries: 0,
      maxRetries: 0,
      createdAt: 0,
      updatedAt: 0,
    };
    act(() => {
      stepHandler?.(
        turnStep({ step: 5, text: 'Proposal draft text.', tools: [], kind: 'proposal' })
      );
    });

    expect(screen.getByText('jobs.prep.proposalTitle')).toBeInTheDocument();
    expect(screen.getByText('Proposal draft text.')).toBeInTheDocument();
    expect(screen.getByText('jobs.prep.stopped.done')).toBeInTheDocument();
    expect(screen.queryByText('jobs.prep.starting')).not.toBeInTheDocument();
  });

  it('reaches `cancelled` via useJob reconciliation when the job was already cancelled before any jobs:event arrived', async () => {
    stubbedJobRecord = {
      id: 'job-1',
      kind: 'ai.generate',
      status: 'cancelled',
      progress: 0,
      payload: {},
      retries: 0,
      maxRetries: 0,
      createdAt: 0,
      updatedAt: 0,
    };
    openModal();
    await clickStart();

    expect(screen.getByText('jobs.prep.stopped.cancelled')).toBeInTheDocument();
    expect(screen.queryByText('jobs.prep.starting')).not.toBeInTheDocument();
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

  it('a successful (done) run shows a working "Prep again" control, not just error/cancelled', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(turnStep({ step: 5, text: 'Proposal.', tools: [], kind: 'proposal' }));
    });
    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'job-1',
        data: { finalText: 'Proposal.', steps: 4, stoppedReason: 'done' },
        ts: 0,
      });
    });
    expect(screen.getByText('jobs.prep.proposalTitle')).toBeInTheDocument();

    // The footer exposes "Prep again" (not the generic "Start" copy) once done.
    const runAgain = screen.getByText('jobs.prep.runAgain').closest('button');
    expect(runAgain).toBeInTheDocument();
    expect(runAgain).not.toBeDisabled();

    mockRunMutateAsync.mockResolvedValueOnce({ jobId: 'job-3' });
    await act(async () => {
      fireEvent.click(runAgain as HTMLButtonElement);
    });

    expect(mockRunMutateAsync).toHaveBeenCalledTimes(2);
    // A fresh run clears the previous proposal until the new one lands.
    expect(screen.queryByText('jobs.prep.proposalTitle')).not.toBeInTheDocument();
    expect(screen.getByText('jobs.prep.starting')).toBeInTheDocument();
  });
});

describe('PrepApplicationPanel — confirm gate (Phase 3)', () => {
  it('a confirm_request suspends the run: shows AgentConfirm, keeps Stop, hides Start', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });

    expect(screen.getByText('jobs.prep.confirm.heading')).toBeInTheDocument();
    expect(screen.getByText('jobs.prep.confirm.tools.saveCoverLetter.summary')).toBeInTheDocument();
    // The run is suspended, not terminal — Stop still works, Start does not show.
    expect(screen.getByText('jobs.prep.stop')).toBeInTheDocument();
    expect(screen.queryByText('jobs.prep.start')).not.toBeInTheDocument();
  });

  it('Approve calls agent.confirm with decision approve, clears the prompt, and the run resumes', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approve'));
    });

    expect(mockConfirmMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '3-save_cover_letter',
      decision: 'approve',
      editedArgs: undefined,
    });
    expect(screen.queryByText('jobs.prep.confirm.heading')).not.toBeInTheDocument();
    // AgentConfirm (and the button just clicked) unmounted — focus is re-parked
    // on the modal title rather than dangling/falling through to <body>.
    expect(document.activeElement).toBe(screen.getByText('jobs.prep.modalTitle'));

    // The loop resumes: a subsequent proposal + job.completed still reaches done.
    act(() => {
      stepHandler?.(
        confirmRequestStep({ step: 5, kind: 'proposal', text: 'Proposal.', confirm: undefined })
      );
    });
    act(() => {
      jobEventHandler?.({
        type: 'job.completed',
        jobId: 'job-1',
        data: { finalText: 'Proposal.', steps: 4, stoppedReason: 'done' },
        ts: 0,
      });
    });
    expect(screen.getByText('jobs.prep.proposalTitle')).toBeInTheDocument();
  });

  it('Deny calls agent.confirm with decision deny and clears the prompt', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.deny'));
    });

    expect(mockConfirmMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '3-save_cover_letter',
      decision: 'deny',
      editedArgs: undefined,
    });
    expect(screen.queryByText('jobs.prep.confirm.heading')).not.toBeInTheDocument();
  });

  it('Edit then Approve sends approveEdited with the edited cover-letter text', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });

    fireEvent.click(screen.getByText('jobs.prep.confirm.edit'));
    const textarea = screen.getByLabelText('jobs.prep.confirm.tools.saveCoverLetter.contentLabel');
    fireEvent.change(textarea, { target: { value: 'Edited draft.' } });

    // Approve is relabeled to `approveEdited` while editing.
    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approveEdited'));
    });

    expect(mockConfirmMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '3-save_cover_letter',
      decision: 'approveEdited',
      editedArgs: { coverLetterText: 'Edited draft.' },
    });
  });

  it('{ ok: false } shows the unavailable message and leaves the run suspended', async () => {
    mockConfirmMutateAsync.mockResolvedValueOnce({ ok: false });
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approve'));
    });

    expect(screen.getByText('jobs.prep.confirm.unavailable')).toBeInTheDocument();
    // Nothing actually resolved — Stop is still the affordance shown, not Start.
    expect(screen.getByText('jobs.prep.stop')).toBeInTheDocument();
    expect(screen.queryByText('jobs.prep.start')).not.toBeInTheDocument();
  });

  it('a server-side CONFIRM_TIMEOUT (turn/proposal step resumes without a resolve) clears the stale confirm card', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });
    expect(screen.getByText('jobs.prep.confirm.heading')).toBeInTheDocument();

    // The backend denied-and-resumed server-side (300s CONFIRM_TIMEOUT) — the
    // renderer never heard a resolve, but a fresh turn for this run arrives.
    act(() => {
      stepHandler?.(turnStep({ tools: ['research_company'], text: 'Researching Acme…' }));
    });

    expect(screen.queryByText('jobs.prep.confirm.heading')).not.toBeInTheDocument();
    expect(screen.getByText('Researching Acme…')).toBeInTheDocument();
  });

  it('a new confirm_request (different callId) replaces the prior pending confirm', async () => {
    openModal();
    await clickStart();

    act(() => {
      stepHandler?.(confirmRequestStep());
    });
    expect(screen.getByText('jobs.prep.confirm.heading')).toBeInTheDocument();

    act(() => {
      stepHandler?.(
        confirmRequestStep({
          step: 6,
          confirm: {
            callId: '6-save_cover_letter',
            tool: 'save_cover_letter',
            args: { coverLetterText: 'Second draft.' },
          },
        })
      );
    });

    expect(screen.getByText('jobs.prep.confirm.heading')).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approve'));
    });

    expect(mockConfirmMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '6-save_cover_letter',
      decision: 'approve',
      editedArgs: undefined,
    });
  });
});
