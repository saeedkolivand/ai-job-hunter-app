/**
 * ImproveResumeRun — the `improve_resume` flow's progress card.
 *
 * What is pinned here:
 *
 *  - **The checklist is the REVIEW sequence**, not the prep one, and it is
 *    built from the step log: a row activates on the last step that matches it.
 *  - **`validate_resume` runs twice** (the post-fix re-check) and stays ONE row
 *    — a second "Checking…" row would claim a step the flow never has.
 *  - **Optional steps appear only if taken.** The prompt rations one of
 *    trim/rewrite per run, so a row always rendered would read as a step that
 *    never happened; `run_quality_pipeline` can also hold ONE step for 90
 *    minutes, so its row says so instead of looking stuck.
 *  - **The gated save is a row too** — announced as a `confirm_request` whose
 *    `tools[]` is empty, which a tool-name-only matcher would miss entirely.
 *  - **Every state offers an action**: Stop while running, Dismiss once it is
 *    not, Approve/Deny while suspended.
 *  - **Focus lands on the card** when it mounts (the run the user just started).
 *
 * Real translations run (no `t: (k) => k` stub): a mistyped or missing key
 * would render the raw key, which the last test asserts never happens.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { AgentStepEvent } from '@ajh/shared';

import type { AgentRunSession } from '@/features/jobs/hooks/useAgentRunSession';

vi.mock('@/services', () => ({
  useAgentConfirm: () => ({
    mutateAsync: vi.fn().mockResolvedValue({ ok: true }),
    isPending: false,
  }),
}));

import { buildImproveRows, ImproveResumeRun } from './index';

const turn = (tools: string[], text = ''): AgentStepEvent => ({
  jobId: 'agent-job-1',
  step: 1,
  text,
  tools,
  denied: [],
  kind: 'turn',
});

const saveConfirm = (): AgentStepEvent => ({
  jobId: 'agent-job-1',
  step: 6,
  text: '',
  // Empty on purpose: this is what the backend actually emits — the tool name
  // lives in `confirm`, not in `tools`.
  tools: [],
  denied: [],
  kind: 'confirm_request',
  confirm: {
    callId: '6-0-save_resume',
    tool: 'save_resume',
    args: { resumeText: 'Summary\nBuilt the deployment pipeline.' },
  },
});

const proposal = (text = 'Fixed two unsourced metrics.'): AgentStepEvent => ({
  jobId: 'agent-job-1',
  step: 7,
  text,
  tools: [],
  denied: [],
  kind: 'proposal',
});

const stop = vi.fn();
const dismiss = vi.fn();
const resolveConfirm = vi.fn();

function session(overrides: Partial<AgentRunSession> = {}): AgentRunSession {
  return {
    state: 'reviewing',
    steps: [],
    stoppedReason: null,
    pendingConfirm: null,
    runJobId: 'agent-job-1',
    error: null,
    busy: true,
    interrupted: false,
    active: true,
    stopRequested: false,
    start: vi.fn(),
    stop,
    dismiss,
    resolveConfirm,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('ImproveResumeRun — the checklist', () => {
  it('lists the review sequence, with the optional steps held back', () => {
    render(<ImproveResumeRun session={session({ steps: [turn(['get_quality_report'])] })} />);

    expect(screen.getByText('Reading the last quality check')).toBeInTheDocument();
    expect(screen.getByText('Checking the generated résumé')).toBeInTheDocument();
    expect(screen.getByText(/Confirming claims against your saved résumé/)).toBeInTheDocument();
    expect(screen.getByText('Proposing the corrected résumé')).toBeInTheDocument();
    // Not the prep flow's rows.
    expect(screen.queryByText(/Researching the company/i)).not.toBeInTheDocument();
    // Rationed steps: not offered as pending work the run may never do.
    expect(screen.queryByText(/Ranking bullets to cut/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Rewriting from your saved résumé/)).not.toBeInTheDocument();
  });

  it('marks the latest step in progress and narrates it', () => {
    render(
      <ImproveResumeRun
        session={session({
          steps: [turn(['get_quality_report']), turn(['validate_resume'], 'Two criticals.')],
        })}
      />
    );

    expect(screen.getByText('Two criticals.')).toBeInTheDocument();
    expect(screen.getByText('In progress')).toBeInTheDocument();
    // The earlier step finished; the untouched ones have not started.
    expect(screen.getByText('Done')).toBeInTheDocument();
    expect(screen.getAllByText('Pending').length).toBeGreaterThan(0);
  });

  // The trap the handoff calls out: the same tool name legitimately appears
  // twice. Key rows on first sight and the re-check either creates a phantom
  // row or leaves the first one stranded.
  it('keeps the re-check on the same row rather than adding a second one', () => {
    render(
      <ImproveResumeRun
        session={session({
          steps: [
            turn(['validate_resume'], 'first pass'),
            turn(['search_candidate_evidence']),
            turn(['validate_resume'], 'after the fixes'),
          ],
        })}
      />
    );

    expect(screen.getAllByText('Checking the generated résumé')).toHaveLength(1);
    // …showing the LATEST of the two runs of it.
    expect(screen.getByText('after the fixes')).toBeInTheDocument();
    expect(screen.queryByText('first pass')).not.toBeInTheDocument();
  });

  // 90 minutes is a healthy step for this tool. A row that says so beats an
  // affordance that nags the user through it.
  it('shows the rewrite step once taken, and warns that it is slow', () => {
    render(<ImproveResumeRun session={session({ steps: [turn(['run_quality_pipeline'])] })} />);
    // Scoped to the checklist: the live region carries the same label (it
    // announces the status change), which is not a second visible row.
    const rows = screen.getByRole('group', { name: /résumé review progress/i });
    expect(
      within(rows).getByText(/Rewriting from your saved résumé — this can take a while/)
    ).toBeInTheDocument();
  });

  it('treats the step that was in flight when a run stopped as interrupted, not done', () => {
    render(
      <ImproveResumeRun
        session={session({
          state: 'cancelled',
          busy: false,
          interrupted: true,
          steps: [turn(['get_quality_report']), turn(['validate_resume'])],
        })}
      />
    );
    expect(screen.getByText('Interrupted')).toBeInTheDocument();
  });
});

describe('ImproveResumeRun — the suspended save', () => {
  it('renders the confirm card with an editable résumé, and marks the save row', () => {
    const step = saveConfirm();
    render(
      <ImproveResumeRun
        session={session({
          state: 'confirming',
          steps: [turn(['validate_resume']), step],
          ...(step.confirm ? { pendingConfirm: step.confirm } : {}),
        })}
      />
    );

    // The row exists even though the step named no tool in `tools[]`.
    expect(screen.getByText('Proposing the corrected résumé')).toBeInTheDocument();
    expect(screen.getByLabelText('Résumé text')).toHaveValue(
      'Summary\nBuilt the deployment pipeline.'
    );
    expect(screen.getByRole('button', { name: /^approve$/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /^deny$/i })).toBeInTheDocument();
  });

  it('announces the suspend once, in the live region', () => {
    const step = saveConfirm();
    render(
      <ImproveResumeRun
        session={session({
          state: 'confirming',
          steps: [step],
          ...(step.confirm ? { pendingConfirm: step.confirm } : {}),
        })}
      />
    );
    expect(screen.getByText(/waiting for your approval before it saves/i)).toBeInTheDocument();
  });
});

describe('ImproveResumeRun — every state has an action', () => {
  it('offers Stop while the run is live, and locks it once requested', async () => {
    const { rerender } = render(<ImproveResumeRun session={session()} />);
    await userEvent.click(screen.getByRole('button', { name: /stop review/i }));
    expect(stop).toHaveBeenCalledTimes(1);

    rerender(<ImproveResumeRun session={session({ stopRequested: true })} />);
    expect(screen.getByRole('button', { name: /stopping/i })).toBeDisabled();
  });

  it('offers Dismiss on a finished run, alongside the summary the agent wrote', async () => {
    render(
      <ImproveResumeRun session={session({ state: 'done', busy: false, steps: [proposal()] })} />
    );
    expect(screen.getByText('What the review found')).toBeInTheDocument();
    expect(screen.getByText('Fixed two unsourced metrics.')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(dismiss).toHaveBeenCalledTimes(1);
  });

  // `job.completed` also fires at the step / token / tool-call ceilings. A card
  // that reads it as success reports a truncated review as a clean finish —
  // with a document the user may then approve believing it was fully checked.
  it('reports a run that stopped at a ceiling as stopped, never as completed', () => {
    render(
      <ImproveResumeRun
        session={session({
          state: 'done',
          busy: false,
          stoppedReason: 'max_tool_calls',
          steps: [proposal('Ran out of tool calls after the second check.')],
        })}
      />
    );

    expect(screen.getByText('Stopped at its tool-call limit')).toBeInTheDocument();
    expect(screen.queryByText('Completed')).not.toBeInTheDocument();
    expect(screen.queryByText(/The review is finished/)).not.toBeInTheDocument();
  });

  // The positive control for the test above — without it, "no Completed" would
  // also pass on a card that never says it.
  it('still reports a clean finish as completed', () => {
    render(
      <ImproveResumeRun
        session={session({
          state: 'done',
          busy: false,
          stoppedReason: 'done',
          steps: [proposal()],
        })}
      />
    );
    expect(screen.getByText('Completed')).toBeInTheDocument();
    expect(screen.getByText(/The review is finished/)).toBeInTheDocument();
  });

  // A reason this build does not know must not be laundered into "Completed".
  it('falls back to a neutral stopped label for an unmapped reason', () => {
    render(
      <ImproveResumeRun
        session={session({ state: 'done', busy: false, stoppedReason: 'some_new_variant' })}
      />
    );
    expect(screen.getByText('Stopped')).toBeInTheDocument();
    expect(screen.queryByText('Completed')).not.toBeInTheDocument();
  });

  it('surfaces a failure as an alert with the backend’s own message', () => {
    render(
      <ImproveResumeRun
        session={session({
          state: 'error',
          busy: false,
          interrupted: true,
          error: 'there is no generated résumé for this job yet',
        })}
      />
    );
    expect(screen.getByRole('alert')).toHaveTextContent(/no generated résumé for this job yet/);
    expect(screen.getByRole('button', { name: /dismiss/i })).toBeInTheDocument();
    // Never Stop on a run that already stopped.
    expect(screen.queryByRole('button', { name: /stop review/i })).not.toBeInTheDocument();
  });

  it('parks focus on the card when it mounts — the run the user just started', () => {
    render(<ImproveResumeRun session={session()} />);
    expect(document.activeElement).toBe(
      screen.getByRole('heading', { name: 'Improving this résumé' })
    );
  });

  it('renders resolved copy everywhere — never a raw i18n key', () => {
    const step = saveConfirm();
    const { container } = render(
      <ImproveResumeRun
        session={session({
          state: 'confirming',
          steps: [turn(['get_quality_report']), turn(['get_trim_suggestions']), step],
          ...(step.confirm ? { pendingConfirm: step.confirm } : {}),
        })}
      />
    );
    expect(container.textContent).not.toMatch(/jobs\.(tailored|prep)\./);
  });
});

describe('buildImproveRows', () => {
  it('holds an untaken optional step out of the list entirely', () => {
    const rows = buildImproveRows([turn(['get_quality_report'])], {
      busy: true,
      interrupted: false,
    });
    expect(rows.map((r) => r.key)).toEqual(['report', 'check', 'evidence', 'save', 'summary']);
  });

  it('inserts a taken optional step in sequence order', () => {
    const rows = buildImproveRows([turn(['get_trim_suggestions'])], {
      busy: true,
      interrupted: false,
    });
    expect(rows.map((r) => r.key)).toEqual([
      'report',
      'check',
      'trim',
      'evidence',
      'save',
      'summary',
    ]);
  });

  // The suspend step names its tool in `confirm`, not in `tools[]`. Match on
  // the tool name alone and the save row silently stays "Pending" through the
  // one decision the whole flow exists to reach.
  it('matches the gated save on the confirm request, whose tools[] is empty', () => {
    const rows = buildImproveRows([saveConfirm()], { busy: true, interrupted: false });
    expect(rows.find((r) => r.key === 'save')?.status).toBe('active');
  });

  it('never calls the latest step of a finished run "in progress"', () => {
    const rows = buildImproveRows([turn(['get_quality_report']), proposal()], {
      busy: false,
      interrupted: false,
    });
    expect(rows.every((r) => r.status !== 'active')).toBe(true);
    expect(rows.find((r) => r.key === 'summary')?.status).toBe('done');
  });
});
