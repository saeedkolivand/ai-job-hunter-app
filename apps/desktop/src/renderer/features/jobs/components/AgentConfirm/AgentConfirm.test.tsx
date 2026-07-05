/**
 * AgentConfirm — the Phase-3 human-in-the-loop confirm prompt.
 *
 * Covers:
 *  - The known `save_cover_letter` tool renders its friendly summary + the
 *    cover-letter text read-only by default.
 *  - Approve (unedited) calls `confirm` with `decision: 'approve'`, no
 *    `editedArgs`, and notifies the parent with `'APPROVE'` once resolved.
 *  - Deny calls `confirm` with `decision: 'deny'` and notifies `'DENY'`.
 *  - Edit moves focus into the now-editable field, relabels Approve, and
 *    offers a Revert-to-original affordance; Edit → change the text →
 *    Approve sends `decision: 'approveEdited'` with `editedArgs` carrying the
 *    edited text (content only).
 *  - `{ ok: false }` shows the "no longer available" copy, does NOT notify
 *    the parent, and reclaims focus onto the heading (not left dangling on
 *    the now-removed action row).
 *  - A rejected `confirm` call (a transport failure, distinct from the
 *    modeled `{ ok: false }`) surfaces a visible error and leaves the actions
 *    usable — never a silent, feedback-free re-enable.
 *  - An unrecognized tool falls back to a generic summary + read-only JSON
 *    args, with no Edit affordance.
 *  - The heading receives focus when a confirm request mounts (decision point).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

import type { AgentConfirmPayload } from '@ajh/shared';

// ── i18n — echo the key, append interpolation params for assertions ─────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, opts?: Record<string, unknown>) => (opts ? `${k}:${JSON.stringify(opts)}` : k),
  }),
}));

// ── @/services — capture the confirm mutation ────────────────────────────────

const mockMutateAsync = vi.fn();
let mockIsPending = false;

vi.mock('@/services', () => ({
  useAgentConfirm: () => ({ mutateAsync: mockMutateAsync, isPending: mockIsPending }),
}));

// ── component under test ──────────────────────────────────────────────────────

import { AgentConfirm } from './index';

const SAVE_COVER_LETTER: AgentConfirmPayload = {
  callId: '3-save_cover_letter',
  tool: 'save_cover_letter',
  args: { coverLetterText: 'Dear hiring manager, ...' },
};

beforeEach(() => {
  mockMutateAsync.mockReset();
  mockMutateAsync.mockResolvedValue({ ok: true });
  mockIsPending = false;
});

describe('AgentConfirm — save_cover_letter (known tool)', () => {
  it('renders the friendly summary and the cover-letter text read-only by default', () => {
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={() => {}} />);

    expect(screen.getByText('jobs.prep.confirm.tools.saveCoverLetter.summary')).toBeInTheDocument();
    const el = screen.getByLabelText('jobs.prep.confirm.tools.saveCoverLetter.contentLabel');
    // Split from the query call — an `as` cast fused onto the call itself gets
    // (incorrectly) autofixed away by no-unnecessary-type-assertion, which
    // infers the query's generic return type FROM that very assertion.
    const textarea = el as HTMLTextAreaElement;
    expect(textarea.value).toBe('Dear hiring manager, ...');
    expect(textarea).toHaveAttribute('readonly');
  });

  it('moves focus to the heading on mount (the decision point)', () => {
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={() => {}} />);
    expect(document.activeElement).toBe(screen.getByText('jobs.prep.confirm.heading'));
  });

  it('Approve (unedited) calls confirm with decision approve and no editedArgs, then resolves APPROVE', async () => {
    const onResolved = vi.fn();
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={onResolved} />);

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approve'));
    });

    expect(mockMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '3-save_cover_letter',
      decision: 'approve',
      editedArgs: undefined,
    });
    expect(onResolved).toHaveBeenCalledWith('APPROVE');
  });

  it('Deny calls confirm with decision deny, then resolves DENY', async () => {
    const onResolved = vi.fn();
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={onResolved} />);

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.deny'));
    });

    expect(mockMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '3-save_cover_letter',
      decision: 'deny',
      editedArgs: undefined,
    });
    expect(onResolved).toHaveBeenCalledWith('DENY');
  });

  it('Edit moves focus into the field, relabels Approve, and sends approveEdited with the edited text', async () => {
    const onResolved = vi.fn();
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={onResolved} />);

    fireEvent.click(screen.getByText('jobs.prep.confirm.edit'));
    const el = screen.getByLabelText('jobs.prep.confirm.tools.saveCoverLetter.contentLabel');
    const textarea = el as HTMLTextAreaElement;
    expect(textarea).not.toHaveAttribute('readonly');
    // Edit is itself a decision point — focus moves into the now-editable field.
    expect(document.activeElement).toBe(textarea);
    fireEvent.change(textarea, { target: { value: 'Edited cover letter text.' } });

    // Approve is relabeled while editing so the changed outcome is explicit.
    expect(screen.queryByText('jobs.prep.confirm.approve')).not.toBeInTheDocument();
    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approveEdited'));
    });

    expect(mockMutateAsync).toHaveBeenCalledWith({
      jobId: 'job-1',
      callId: '3-save_cover_letter',
      decision: 'approveEdited',
      editedArgs: { coverLetterText: 'Edited cover letter text.' },
    });
    expect(onResolved).toHaveBeenCalledWith('APPROVE');
  });

  it('Revert restores the original text and exits edit mode', () => {
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={() => {}} />);

    fireEvent.click(screen.getByText('jobs.prep.confirm.edit'));
    const el = screen.getByLabelText('jobs.prep.confirm.tools.saveCoverLetter.contentLabel');
    const textarea = el as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: 'A regretted edit.' } });

    fireEvent.click(screen.getByText('jobs.prep.confirm.revert'));

    expect(textarea.value).toBe('Dear hiring manager, ...');
    expect(textarea).toHaveAttribute('readonly');
    // Back to view-only — Edit is offered again, Revert is gone.
    expect(screen.getByText('jobs.prep.confirm.edit')).toBeInTheDocument();
    expect(screen.queryByText('jobs.prep.confirm.revert')).not.toBeInTheDocument();
  });

  it('{ ok: false } shows the unavailable message, does not notify the parent, and reclaims focus onto the heading', async () => {
    mockMutateAsync.mockResolvedValueOnce({ ok: false });
    const onResolved = vi.fn();
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={onResolved} />);

    // Move focus away from the heading first so the reclaim is actually exercised.
    const el = screen.getByLabelText('jobs.prep.confirm.tools.saveCoverLetter.contentLabel');
    (el as HTMLTextAreaElement).focus();
    expect(document.activeElement).toBe(el);

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approve'));
    });

    expect(screen.getByText('jobs.prep.confirm.unavailable')).toBeInTheDocument();
    expect(onResolved).not.toHaveBeenCalled();
    // The action row is gone — nothing left to click on a stale prompt.
    expect(screen.queryByText('jobs.prep.confirm.approve')).not.toBeInTheDocument();
    // Focus doesn't dangle on the now-removed action row / fall to <body>.
    expect(document.activeElement).toBe(screen.getByText('jobs.prep.confirm.heading'));
  });

  it('a rejected confirm call surfaces a visible error and leaves the actions usable (no silent re-enable)', async () => {
    mockMutateAsync.mockRejectedValueOnce(new Error('network down'));
    const onResolved = vi.fn();
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={onResolved} />);

    await act(async () => {
      fireEvent.click(screen.getByText('jobs.prep.confirm.approve'));
    });

    expect(screen.getByRole('alert')).toHaveTextContent('jobs.prep.confirm.error');
    expect(onResolved).not.toHaveBeenCalled();
    // Re-enabled for a retry, but WITH visible feedback — not a silent no-op.
    expect(screen.getByText('jobs.prep.confirm.approve').closest('button')).not.toBeDisabled();
    expect(screen.getByText('jobs.prep.confirm.deny').closest('button')).not.toBeDisabled();
  });

  it('disables Approve/Deny/Edit while the confirm mutation is in flight', () => {
    mockIsPending = true;
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={() => {}} />);

    expect(screen.getByText('jobs.prep.confirm.approve').closest('button')).toBeDisabled();
    expect(screen.getByText('jobs.prep.confirm.deny').closest('button')).toBeDisabled();
    expect(screen.getByText('jobs.prep.confirm.edit').closest('button')).toBeDisabled();
  });

  it('shows a "submitting" status while the confirm mutation is in flight', () => {
    mockIsPending = true;
    render(<AgentConfirm jobId="job-1" confirm={SAVE_COVER_LETTER} onResolved={() => {}} />);

    expect(screen.getByText('jobs.prep.confirm.submitting')).toBeInTheDocument();
  });
});

describe('AgentConfirm — unrecognized tool (generic fallback)', () => {
  const UNKNOWN: AgentConfirmPayload = {
    callId: '5-unknown_tool',
    tool: 'unknown_tool',
    args: { foo: 'bar' },
  };

  it('shows a generic summary + read-only JSON args, with no Edit affordance', () => {
    render(<AgentConfirm jobId="job-1" confirm={UNKNOWN} onResolved={() => {}} />);

    expect(
      screen.getByText('jobs.prep.confirm.genericSummary:{"tool":"unknown_tool"}')
    ).toBeInTheDocument();
    const el = screen.getByLabelText('jobs.prep.confirm.rawArgsLabel');
    const textarea = el as HTMLTextAreaElement;
    expect(textarea.value).toBe(JSON.stringify({ foo: 'bar' }, null, 2));
    expect(textarea).toHaveAttribute('readonly');
    expect(screen.queryByText('jobs.prep.confirm.edit')).not.toBeInTheDocument();
  });
});
