/**
 * The mounted job-tools component's page gating: the fields probe (Form group +
 * `onAnswerToolsVisibility`), the trust gate's effect on rendering, the popup's
 * `setImportLabel` / `reset` seam, and per-instance isolation.
 */

import { describe, expect, it, vi } from 'vitest';

import type { PopupRequest, PopupResponse } from '../../lib/messages';
import { IMPORT_LABEL_DEFAULT, JOB_TOOLS_GATED_LINE, mountJobTools } from '../job-tools';
import { answerState, btn, flush, mount } from '../test-support';

const probe = (hasFormFields: boolean, hasAnswerFields = hasFormFields): PopupResponse => ({
  ok: true,
  kind: 'fieldsProbe',
  hasFormFields,
  hasAnswerFields,
});
const rejecting = async (): Promise<never> => {
  throw new Error('message channel closed');
};
const el = (host: HTMLElement, selector: string) => host.querySelector<HTMLElement>(selector)!;
const formGroupHidden = (host: HTMLElement): boolean => el(host, '#group-form').hidden;

describe('checkPage — fields probe (Form group + onAnswerToolsVisibility)', () => {
  it('hides the Form group and forwards showAnswerTools:false when the probe finds no fields', async () => {
    const { host, send, onAnswerToolsVisibility, view } = mount(async () => probe(false));
    view.checkPage();
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'fieldsProbe' });
    expect(formGroupHidden(host)).toBe(true);
    expect(onAnswerToolsVisibility).toHaveBeenCalledWith(false);
  });

  it('fails OPEN on a transport rejection', async () => {
    const { host, onAnswerToolsVisibility, view } = mount(rejecting);
    view.checkPage();
    await flush();

    expect(formGroupHidden(host)).toBe(false);
    expect(onAnswerToolsVisibility).toHaveBeenCalledWith(true);
  });

  it('a stale response from an earlier checkPage() call must not clobber a newer one (generation guard)', async () => {
    let resolveFirst: ((res: PopupResponse) => void) | undefined;
    const first = new Promise<PopupResponse>((resolve) => {
      resolveFirst = resolve;
    });
    const send = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce(probe(true));
    const { host, view } = mount(send);

    view.checkPage(); // first call — left pending
    view.checkPage(); // second call — resolves first, "wins"
    await flush();

    expect(formGroupHidden(host)).toBe(false);

    // The first call's stale "no fields" response arrives late — must be a no-op.
    resolveFirst?.(probe(false));
    await flush();

    expect(formGroupHidden(host)).toBe(false);
  });
});

describe('render — the trust gate', () => {
  const gated = (host: HTMLElement) => el(host, '#job-tools-gated');
  const active = (host: HTMLElement) => el(host, '#job-tools-active');

  it('starts trusted (active controls shown, gated line hidden) before any state is known', () => {
    const { host } = mount();
    expect(gated(host).hidden).toBe(true);
    expect(active(host).hidden).toBe(false);
  });

  it('replaces the four controls with the gated line for a navigated (pageChanged) tab', () => {
    const { host, view } = mount();
    view.render(answerState({ pageChanged: true }));

    expect(gated(host).hidden).toBe(false);
    expect(gated(host).textContent).toBe(JOB_TOOLS_GATED_LINE);
    expect(active(host).hidden).toBe(true);
  });

  it('replaces the four controls with the SAME gated line when no record exists at all', () => {
    const { host, view } = mount();
    view.render(null);

    expect(gated(host).hidden).toBe(false);
  });

  it('unblocks the controls again once a fresh, untraveled record arrives (a valid gesture landed)', () => {
    const { host, view } = mount();
    view.render(answerState({ pageChanged: true }));
    expect(active(host).hidden).toBe(true);

    view.render(answerState({ pageChanged: false }));

    expect(gated(host).hidden).toBe(true);
    expect(active(host).hidden).toBe(false);
  });

  it('does not call send for checkPage while untrusted', async () => {
    const { send, view } = mount();
    view.render(answerState({ pageChanged: true }));
    view.checkPage();
    await flush();

    expect(send).not.toHaveBeenCalled();
  });

  it('re-runs the fields probe automatically when regaining trust while mounted (the panel never remounts on a toolbar-click re-grant)', async () => {
    const { send, view } = mount(async () => probe(true));
    view.render(answerState({ pageChanged: true })); // untrusted
    view.render(answerState({ pageChanged: false })); // regained trust
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'fieldsProbe' });
  });

  it('resets tab-A page-specific state (match-fit score, Form group) when the panel switches to a DIFFERENT already-trusted tab', async () => {
    const { host, send, view } = mount(async (req) =>
      req.kind === 'matchLive'
        ? {
            ok: true,
            kind: 'matchLive',
            result: {
              ok: true,
              combined: 90,
              ats: 80,
              gaps: [],
              resumeName: 'My Resume',
              scoreSource: 'keyword',
            },
          }
        : probe(true)
    );

    // Tab A: trusted, and the user ran Check fit — a score is on screen.
    view.render(answerState({ tabId: 1, pageChanged: false }));
    btn(host, '#btn-check-fit').click();
    await flush();
    expect(el(host, '#match-result').hidden).toBe(false);

    // The panel switches to tab B — ALSO already trusted per its OWN
    // AnswerState record. `isPageTrusted` returns `true` for both, so a
    // trust-flag-only dedup would wrongly skip resetting anything here.
    send.mockClear();
    view.render(answerState({ tabId: 2, pageChanged: false }));

    // Tab A's score must not leak onto tab B.
    expect(el(host, '#match-result').hidden).toBe(true);
    expect(el(host, '#match-result').textContent).toBe('');
    // The Form group re-probes for tab B rather than keeping tab A's answer.
    expect(send).toHaveBeenCalledWith({ kind: 'fieldsProbe' });
  });
});

// ── setImportLabel / reset (the popup's own appliedCheck seam) ───────────────

describe('setImportLabel and reset', () => {
  it('setImportLabel overrides the Import button text', () => {
    const { host, view } = mount();
    view.setImportLabel('Re-import / update');
    expect(btn(host, '#btn-import').textContent).toBe('Re-import / update');
  });

  it('reset restores the default Import label, hides the match card, and re-shows the Form group', () => {
    const { host, onAnswerToolsVisibility, view } = mount();
    view.setImportLabel('Re-import / update');
    view.reset();

    expect(btn(host, '#btn-import').textContent).toBe(IMPORT_LABEL_DEFAULT);
    expect(el(host, '#match-result').hidden).toBe(true);
    expect(formGroupHidden(host)).toBe(false);
    expect(onAnswerToolsVisibility).toHaveBeenCalledWith(true);
  });
});

// ── per-instance isolation ────────────────────────────────────────────────────

describe('two mounted instances (popup + panel) never share state', () => {
  it('a generation bump in one instance does not affect the other', async () => {
    let resolveA: ((res: PopupResponse) => void) | undefined;
    const pendingA = new Promise<PopupResponse>((resolve) => {
      resolveA = resolve;
    });
    const sendA = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockReturnValueOnce(pendingA);
    const sendB = vi
      .fn<(req: PopupRequest) => Promise<PopupResponse>>()
      .mockResolvedValueOnce(probe(false));
    const hostA = document.createElement('div');
    const hostB = document.createElement('div');
    const viewA = mountJobTools(hostA, { send: sendA });
    const viewB = mountJobTools(hostB, { send: sendB });

    viewA.checkPage();
    viewB.checkPage(); // a DIFFERENT instance's generation counter
    await flush();

    // B's own probe resolved and hid its OWN Form group.
    expect(formGroupHidden(hostB)).toBe(true);
    // A's is still pending, so A's Form group is untouched (still visible).
    expect(formGroupHidden(hostA)).toBe(false);

    resolveA?.(probe(false));
    await flush();

    expect(formGroupHidden(hostA)).toBe(true);
    // B's own generation was never bumped by A's calls.
    expect(sendB).toHaveBeenCalledTimes(1);
  });
});
