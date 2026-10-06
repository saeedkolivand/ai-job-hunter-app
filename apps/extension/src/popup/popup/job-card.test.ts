/**
 * The popup's page-context card and its connected-only gating: `view-import` and
 * the "Unpair this device" group toggle off a real status push through the SAME
 * connection-status module, the `appliedCheck` / `fieldsProbe` auto-checks that
 * fire on ENTERING `connected`, "Mark as applied", and unpair.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', async () => (await import('./browser-mock')).popupBrowserMock());
vi.mock('../../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

import { bootPopup, byId, flush, statusPusher } from './test-support';

await bootPopup();
const push = statusPusher();
const sendMessageMock = vi.mocked(browser.runtime.sendMessage);

const hidden = (id: string): boolean => byId<HTMLElement>(id).hidden;
const textOf = (id: string): string | null => byId<HTMLElement>(id).textContent;

/** An `appliedCheck` reply carrying `result`. */
const applied = (result: Record<string, unknown>) =>
  ({ ok: true, kind: 'appliedCheck', result }) as const;
const NEUTRAL_APPLIED_CHECK = applied({ found: false });

describe('view-import + unpair-group gating (via the real connection-status module)', () => {
  it('shows view-import only for connected, and hides it (resetting job/job-card) otherwise', () => {
    push('connected');
    expect(hidden('view-import')).toBe(false);

    push('app_not_running');
    expect(hidden('view-import')).toBe(true);
    expect(hidden('job-card')).toBe(true);
  });

  it('shows "Unpair this device" only while a pairing token is stored, independent of phase', () => {
    push('not_paired', false);
    expect(hidden('unpair-group')).toBe(true);

    push('connected', true);
    expect(hidden('unpair-group')).toBe(false);

    push('app_not_running', false);
    expect(hidden('unpair-group')).toBe(true);
  });
});

// ── appliedCheck auto-check (fire-and-forget on entering `connected`) ──────────

describe('appliedCheck auto-check', () => {
  beforeEach(() => {
    sendMessageMock.mockReset();
    // Force a non-connected phase first so the next `push('connected')` below is
    // a genuine transition regardless of what an earlier test left behind — the
    // auto-check only fires on ENTERING `connected`, not on a repeated push.
    push('searching');
  });

  it('sends an appliedCheck request and renders the job-card chip with the relabeled button', async () => {
    sendMessageMock.mockResolvedValueOnce(
      applied({ found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) })
    );

    push('connected');
    await flush();

    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'appliedCheck' });
    expect(hidden('job-card')).toBe(false);
    expect(hidden('applied-status')).toBe(false);
    expect(textOf('applied-status')).toMatch(/^Applied /);
    expect(textOf('btn-import')).toBe('Re-import / update');
    // Already applied — the mark-applied button has nothing left to do.
    expect(hidden('btn-mark-applied')).toBe(true);
  });

  it('shows the mark-applied button for a found+saved result', async () => {
    sendMessageMock.mockResolvedValueOnce(applied({ found: true, status: 'saved' }));

    push('connected');
    await flush();

    expect(hidden('btn-mark-applied')).toBe(false);
  });

  it('renders nothing and keeps the default button label when not found', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK);

    push('connected');
    await flush();

    expect(hidden('job-card')).toBe(true);
    expect(textOf('btn-import')).toBe('Import this job');
  });

  it('soft-fails silently (card stays hidden, default label, no thrown error) when the request rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    push('connected');
    await flush();

    expect(hidden('job-card')).toBe(true);
    expect(textOf('btn-import')).toBe('Import this job');
  });

  it('does not re-fire the check on a repeated connected push with no intervening phase change', async () => {
    sendMessageMock.mockResolvedValueOnce(applied({ found: true, status: 'saved' }));
    push('connected');
    await flush();
    // Entering `connected` fires three fire-and-forget auto-checks — appliedCheck,
    // fieldsProbe (Form group gating) and answerScan (the notice line's data).
    expect(sendMessageMock).toHaveBeenCalledTimes(3);
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'answerScan' });
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'fieldsProbe' });

    sendMessageMock.mockClear();
    push('connected'); // same phase again — not a transition
    await flush();
    expect(sendMessageMock).not.toHaveBeenCalled();
  });

  it('clears the stale card + button label on leaving connected, with no flash before the next check resolves', async () => {
    sendMessageMock.mockResolvedValueOnce(
      applied({ found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) })
    );

    push('connected');
    await flush();

    expect(hidden('job-card')).toBe(false);
    expect(textOf('btn-import')).toBe('Re-import / update');
    expect(hidden('btn-mark-applied')).toBe(true); // job A is already applied

    // Desktop drops the connection — job A's stale card must not survive.
    push('app_not_running');
    expect(hidden('job-card')).toBe(true);
    expect(textOf('btn-import')).toBe('Import this job');
    expect(hidden('btn-mark-applied')).toBe(true);

    // Reconnect for job B — before its own check resolves, the pre-resolve
    // state must already be clean (no lingering job-A content while it's in flight).
    sendMessageMock.mockResolvedValueOnce(applied({ found: true, status: 'saved' }));
    push('connected');
    expect(hidden('job-card')).toBe(true);
    expect(textOf('btn-import')).toBe('Import this job');
    expect(hidden('btn-mark-applied')).toBe(true);
    await flush();
    // Job B's check resolves as found+saved — the button appears for it.
    expect(hidden('btn-mark-applied')).toBe(false);
  });

  it('ignores a stale in-flight response that resolves after a newer check has already rendered', async () => {
    // Check A starts on entering `connected` for job A, but its response never
    // resolves yet (simulates it still being in flight when a reconnect fires).
    let resolveA: ((res: unknown) => void) | undefined;
    const pendingA = new Promise((resolve) => {
      resolveA = resolve;
    });
    // Answer by request KIND, not by call order: entering `connected` fires
    // `appliedCheck` + `fieldsProbe` + `answerScan` together, so an
    // order-queued mock silently hands check B's response to whichever sibling
    // request happens to be issued second. Keying on the kind pins what this
    // test is actually about — the FIRST appliedCheck stays in flight, the
    // SECOND one resolves for job B.
    let appliedChecks = 0;
    sendMessageMock.mockImplementation((req: unknown) => {
      const kind = (req as { kind: string }).kind;
      if (kind !== 'appliedCheck') return Promise.resolve({ ok: false, error: 'not under test' });
      appliedChecks += 1;
      if (appliedChecks === 1) return pendingA;
      return Promise.resolve(applied({ found: true, status: 'saved', title: 'Job B' }));
    });
    push('connected');

    // Disconnect → reconnect: a fresh, edge-triggered check B starts for job B
    // and resolves before A does.
    push('app_not_running');
    push('connected');
    await flush();

    expect(textOf('job-card-title')).toBe('Job B');
    expect(textOf('applied-status')).toBe('Saved');
    expect(textOf('btn-import')).toBe('Re-import / update');

    // Check A finally resolves late (found:false for job A) — it must NOT
    // overwrite the already-rendered job B result.
    resolveA?.(NEUTRAL_APPLIED_CHECK);
    await flush();

    expect(textOf('job-card-title')).toBe('Job B');
    expect(textOf('applied-status')).toBe('Saved');
    expect(textOf('btn-import')).toBe('Re-import / update');
  });
});

// ── fieldsProbe auto-check (fire-and-forget on entering `connected`) ──────────
// Gates the Form group (#group-form) on "does this page have fillable form
// fields?". Runs ALONGSIDE the appliedCheck auto-check above on the SAME
// transition — the first queued sendMessage response answers appliedCheck
// (code calls it first), the second answers fieldsProbe. There is no
// Answer-tools disclosure in the popup to gate (`onAnswerToolsVisibility` is
// intentionally omitted) — only #group-form.

describe('fieldsProbe auto-check (#group-form gating)', () => {
  const probe = (has: boolean) =>
    ({ ok: true, kind: 'fieldsProbe', hasFormFields: has, hasAnswerFields: has }) as const;

  beforeEach(() => {
    sendMessageMock.mockReset();
    // Force a genuine transition for the next push('connected') below.
    push('searching');
  });

  it('shows the Form group when the probe finds fillable fields', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce(probe(true));

    push('connected');
    await flush();

    expect(hidden('group-form')).toBe(false);
  });

  it('hides the Form group when the probe finds no fillable fields at all', async () => {
    sendMessageMock
      .mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK)
      .mockResolvedValueOnce(probe(false));

    push('connected');
    await flush();

    expect(hidden('group-form')).toBe(true);
  });

  it('fails OPEN (shows the Form group) when the probe request rejects', async () => {
    sendMessageMock
      .mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK)
      .mockRejectedValueOnce(new Error('message channel closed'));

    push('connected');
    await flush();

    expect(hidden('group-form')).toBe(false);
  });

  it('re-shows the Form group on a fresh page after a previous page hid it (no stale hide across a reconnect)', async () => {
    sendMessageMock
      .mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK)
      .mockResolvedValueOnce(probe(false));
    push('connected');
    await flush();
    expect(hidden('group-form')).toBe(true);

    // Disconnect (leaving `connected` resets to the fail-open default) then
    // reconnect for a fresh page whose own probe hasn't resolved yet.
    push('app_not_running');
    expect(hidden('group-form')).toBe(false);

    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce(probe(true));
    push('connected');
    await flush();
    expect(hidden('group-form')).toBe(false);
  });
});

// ── doMarkApplied (#btn-mark-applied) ─────────────────────────────────────────

describe('doMarkApplied (#btn-mark-applied)', () => {
  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLButtonElement>('btn-mark-applied').hidden = false;
    byId<HTMLButtonElement>('btn-mark-applied').disabled = false;
    byId<HTMLParagraphElement>('import-msg').textContent = '';
  });

  it('shows "Marking as applied…" then re-fires the auto-check on success, hiding the button', async () => {
    sendMessageMock
      .mockResolvedValueOnce({
        ok: true,
        kind: 'statusUpdate',
        result: { ok: true, applicationId: 'app-1', status: 'applied' },
      })
      // The success-path re-fire of runAppliedAutoCheck sends a SECOND
      // request — the same generation-guarded path every other render goes
      // through, never a hand-rolled DOM update.
      .mockResolvedValueOnce(applied({ found: true, status: 'applied' }));

    const btn = byId<HTMLButtonElement>('btn-mark-applied');
    btn.click();
    expect(btn.disabled).toBe(true);
    expect(textOf('import-msg')).toBe('Marking as applied…');

    await flush();
    await flush();

    expect(textOf('import-msg')).toBe('Marked as applied.');
    expect(sendMessageMock).toHaveBeenNthCalledWith(1, { kind: 'statusUpdate' });
    expect(sendMessageMock).toHaveBeenNthCalledWith(2, { kind: 'appliedCheck' });
    // The re-fired auto-check's found+applied result hides the button.
    expect(btn.hidden).toBe(true);
  });

  it('surfaces the desktop refusal text and re-enables the button (errors ARE shown, unlike the passive check)', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'statusUpdate',
      result: { ok: false, error: "couldn't find a saved job for this page" },
    });

    const btn = byId<HTMLButtonElement>('btn-mark-applied');
    btn.click();
    await flush();

    expect(textOf('import-msg')).toBe("couldn't find a saved job for this page");
    expect(btn.disabled).toBe(false);
    // No auto-check re-fire on failure — only one request went out.
    expect(sendMessageMock).toHaveBeenCalledTimes(1);
  });

  it('shows a retry message and re-enables the button when sendMessage rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    const btn = byId<HTMLButtonElement>('btn-mark-applied');
    btn.click();
    await flush();

    expect(textOf('import-msg')).toBe('Could not mark this job as applied. Please retry.');
    expect(btn.disabled).toBe(false);
  });
});

// ── unpair (#btn-unpair, reachable via the "?" help popover) ────────────────

describe('unpair (#btn-unpair, #unpair-group hasToken-gated)', () => {
  it('clears the token and returns to the pairing view', async () => {
    sendMessageMock.mockReset();
    sendMessageMock
      .mockResolvedValueOnce({ ok: true, kind: 'token' }) // clearToken
      .mockResolvedValueOnce({
        ok: true,
        kind: 'status',
        status: { phase: 'not_paired', port: 1, hasToken: false },
      });
    byId<HTMLElement>('view-pair').hidden = true;

    byId<HTMLButtonElement>('btn-unpair').click();
    await flush();

    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'clearToken' });
    expect(hidden('view-pair')).toBe(false);
  });

  it('shows the "Unpair this device" group only while a pairing token is stored', () => {
    push('not_paired', false);
    expect(hidden('unpair-group')).toBe(true);

    push('connected', true);
    expect(hidden('unpair-group')).toBe(false);

    push('app_not_running', false);
    expect(hidden('unpair-group')).toBe(true);
  });
});
