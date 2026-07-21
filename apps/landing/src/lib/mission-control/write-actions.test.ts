import { describe, expect, it, vi } from 'vitest';

import { performWriteAction, WRITE_ACTIONS, type WriteAction } from './write-actions';

function actionById(id: string): WriteAction {
  const action = WRITE_ACTIONS.find((a) => a.id === id);
  if (!action) throw new Error(`no such action: ${id}`);
  return action;
}

describe('write-action registry', () => {
  it('contains NO pull-request merge action anywhere', () => {
    for (const action of WRITE_ACTIONS) {
      expect(action.id).not.toMatch(/merge/i);
      expect(action.label).not.toMatch(/merge/i);
      const samplePath = action.path({ issueNumber: 1, runId: 1, label: 'x', comment: 'x' });
      expect(samplePath).not.toContain('/merge');
      expect(samplePath).not.toContain('/pulls/');
    }
  });

  it('every action is a POST or PATCH (never a raw PUT/DELETE)', () => {
    for (const action of WRITE_ACTIONS) {
      expect(['POST', 'PATCH']).toContain(action.method);
    }
  });
});

describe('performWriteAction — the confirm gate', () => {
  it('makes NO request when the user declines the confirm dialog', async () => {
    const request = vi.fn();
    const confirm = vi.fn().mockResolvedValue(false);

    const outcome = await performWriteAction(
      actionById('close-issue'),
      { issueNumber: 42 },
      { token: 't', confirm, request }
    );

    expect(outcome).toEqual({ status: 'cancelled' });
    expect(confirm).toHaveBeenCalledOnce();
    expect(request).not.toHaveBeenCalled();
  });

  it('requests exactly once — with the right path/method/body/token — only after approval', async () => {
    const request = vi.fn().mockResolvedValue({ ok: true, status: 200 });
    const confirm = vi.fn().mockResolvedValue(true);

    const outcome = await performWriteAction(
      actionById('close-issue'),
      { issueNumber: 42 },
      { token: 'tkn', confirm, request }
    );

    expect(outcome).toEqual({ status: 'done', result: { ok: true, status: 200 } });
    expect(request).toHaveBeenCalledOnce();
    expect(request).toHaveBeenCalledWith('/issues/42', 'PATCH', { state: 'closed' }, 'tkn');
  });

  it('confirms BEFORE requesting (gate cannot be bypassed by ordering)', async () => {
    const order: string[] = [];
    const confirm = vi.fn().mockImplementation(async () => {
      order.push('confirm');
      return true;
    });
    const request = vi.fn().mockImplementation(async () => {
      order.push('request');
      return { ok: true, status: 200 };
    });

    await performWriteAction(
      actionById('rerun-failed'),
      { runId: 7 },
      { token: 't', confirm, request }
    );
    expect(order).toEqual(['confirm', 'request']);
  });

  it('dispatches the release workflow on main with a ref body', async () => {
    const request = vi.fn().mockResolvedValue({ ok: true, status: 204 });
    await performWriteAction(
      actionById('dispatch-release'),
      {},
      { token: 't', confirm: () => Promise.resolve(true), request }
    );
    expect(request).toHaveBeenCalledWith(
      '/actions/workflows/release.yml/dispatches',
      'POST',
      { ref: 'main' },
      't'
    );
  });
});
