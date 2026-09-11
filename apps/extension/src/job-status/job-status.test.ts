import { describe, expect, it, vi } from 'vitest';

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
});
