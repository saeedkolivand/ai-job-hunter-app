import { describe, expect, it, vi } from 'vitest';

import type { PopupRequest, PopupResponse } from '../lib/messages';
import { mountJobStatus, resolveJobStatusView, stageIndex } from './job-status';

describe('stageIndex (pure)', () => {
  it('maps a known status to its stage index', () => {
    expect(stageIndex('saved')).toBe(0);
    expect(stageIndex('applied')).toBe(1);
    expect(stageIndex('interviewing')).toBe(2);
    expect(stageIndex('offer')).toBe(3);
  });

  it('returns -1 for an unknown/unmapped status', () => {
    expect(stageIndex('rejected')).toBe(-1);
    expect(stageIndex(undefined)).toBe(-1);
  });
});

describe('resolveJobStatusView (pure)', () => {
  it('returns null for a non-appliedCheck response', () => {
    expect(resolveJobStatusView({ ok: true, kind: 'token' })).toBeNull();
  });

  it('returns null when ok is false', () => {
    expect(resolveJobStatusView({ ok: false, error: 'boom' })).toBeNull();
  });

  it('returns null when not found or the result carries an error', () => {
    expect(
      resolveJobStatusView({ ok: true, kind: 'appliedCheck', result: { found: false } })
    ).toBeNull();
    expect(
      resolveJobStatusView({
        ok: true,
        kind: 'appliedCheck',
        result: { found: true, error: 'malformed' },
      })
    ).toBeNull();
  });

  it('defaults an absent status to saved', () => {
    const view = resolveJobStatusView({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, title: 'Senior Rust Engineer' },
    });
    expect(view).toEqual({
      title: 'Senior Rust Engineer',
      chipText: 'Saved',
      currentStageIndex: 0,
    });
  });

  it('includes the applied date in the chip when present', () => {
    const view = resolveJobStatusView({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) },
    });
    expect(view?.chipText).toMatch(/^Applied .+$/);
    expect(view?.currentStageIndex).toBe(1);
  });

  it('has a null title when the desktop sent none', () => {
    const view = resolveJobStatusView({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved' },
    });
    expect(view?.title).toBeNull();
  });

  it('gives an unmapped terminal status its own neutral, capitalized label rather than "Saved"', () => {
    const rejected = resolveJobStatusView({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'rejected' },
    });
    expect(rejected).toEqual({ title: null, chipText: 'Rejected', currentStageIndex: -1 });

    const ghosted = resolveJobStatusView({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'ghosted' },
    });
    expect(ghosted).toEqual({ title: null, chipText: 'Ghosted', currentStageIndex: -1 });
  });
});

describe('mountJobStatus', () => {
  function mount() {
    const host = document.createElement('div');
    const send = vi.fn();
    const handle = mountJobStatus(host, { send });
    return { host, send, handle };
  }

  it('starts hidden', () => {
    const { host } = mount();
    expect(host.querySelector<HTMLElement>('.card')!.hidden).toBe(true);
  });

  it('renders the card on a found result', async () => {
    const { host, send, handle } = mount();
    send.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved', title: 'Senior Rust Engineer' },
    });

    await handle.refresh();

    expect(send).toHaveBeenCalledWith({ kind: 'appliedCheck' });
    const card = host.querySelector<HTMLElement>('.card')!;
    expect(card.hidden).toBe(false);
    expect(card.textContent).toContain('Senior Rust Engineer');
    expect(card.textContent).toContain('Saved');
    const stages = card.querySelectorAll('.stage');
    expect(stages).toHaveLength(4);
    expect(stages[0]?.classList.contains('current')).toBe(true);
  });

  it('hides the card when nothing is found', async () => {
    const { host, send, handle } = mount();
    send.mockResolvedValueOnce({ ok: true, kind: 'appliedCheck', result: { found: false } });

    await handle.refresh();

    expect(host.querySelector<HTMLElement>('.card')!.hidden).toBe(true);
  });

  it('hides the card rather than throwing when the request rejects', async () => {
    const { host, send, handle } = mount();
    send.mockRejectedValueOnce(new Error('message channel closed'));

    await expect(handle.refresh()).resolves.toBeUndefined();
    expect(host.querySelector<HTMLElement>('.card')!.hidden).toBe(true);
  });

  it('reset() hides and clears the card', async () => {
    const { host, send, handle } = mount();
    send.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved', title: 'X' },
    });
    await handle.refresh();

    handle.reset();

    const card = host.querySelector<HTMLElement>('.card')!;
    expect(card.hidden).toBe(true);
    expect(card.textContent).toBe('');
  });

  it('renders the stage strip with no stage highlighted for an unmapped status', async () => {
    const { host, send, handle } = mount();
    send.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'rejected', title: 'X' },
    });

    await handle.refresh();

    const card = host.querySelector<HTMLElement>('.card')!;
    expect(card.textContent).toContain('Rejected');
    expect(card.querySelectorAll('.stage.current')).toHaveLength(0);
  });

  it('a stale in-flight refresh() must not clobber a newer render (generation guard)', async () => {
    let resolveFirst: ((res: PopupResponse) => void) | undefined;
    const first = new Promise<PopupResponse>((resolve) => {
      resolveFirst = resolve;
    });
    const send = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce({
        ok: true,
        kind: 'appliedCheck',
        result: { found: true, status: 'applied', title: 'Newer' },
      });
    const host = document.createElement('div');
    const handle = mountJobStatus(host, { send });

    const p1 = handle.refresh(); // first call — left pending
    const p2 = handle.refresh(); // second call — resolves immediately, "wins"
    await p2;

    const card = host.querySelector<HTMLElement>('.card')!;
    expect(card.textContent).toContain('Newer');

    // The first call's stale response arrives late — must be a no-op.
    resolveFirst?.({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved', title: 'Stale' },
    });
    await p1;

    expect(card.textContent).toContain('Newer');
    expect(card.textContent).not.toContain('Stale');
  });

  it('reset() also bumps the generation, so a refresh() left in flight cannot resurrect the card', async () => {
    let resolvePending: ((res: PopupResponse) => void) | undefined;
    const pending = new Promise<PopupResponse>((resolve) => {
      resolvePending = resolve;
    });
    const send = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockReturnValueOnce(pending);
    const host = document.createElement('div');
    const handle = mountJobStatus(host, { send });

    const p = handle.refresh(); // left pending
    handle.reset();

    resolvePending?.({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved', title: 'Stale' },
    });
    await p;

    expect(host.querySelector<HTMLElement>('.card')!.hidden).toBe(true);
  });
});
