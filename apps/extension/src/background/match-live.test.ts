/** `matchLive` ("Check fit") and the best-effort on-page fit badge it paints (PR3 §B.3). */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { FitBadgeView } from '../lib/fit-badge';
import {
  DESKTOP_DOWN,
  executeScriptMock,
  FAKE_TOKEN,
  fitBadgeRenderCall,
  flush,
  getTokenMock,
  matchOk,
  mockClient,
  paired,
  POSTING_URL,
  RELOAD_HINT,
  resetMocks,
  scriptResults,
  send,
  setShowFitBadge,
  tabSequence,
} from './test-support';

beforeEach(resetMocks);

const PAGE_HTML = '<html>job</html>';
const badgeInjected = () => expect.objectContaining({ files: ['fit-badge.js'] });

describe('matchLive request — capture then send; errors are NOT folded', () => {
  it('captures content.js, sends { url, html }, and returns the success result', async () => {
    paired();
    scriptResults(PAGE_HTML);
    const result = matchOk({ combined: 72, gaps: ['kubernetes'] });
    mockClient.matchLive.mockResolvedValue(result);

    const res = await send({ kind: 'matchLive' });

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['content.js'] })
    );
    expect(mockClient.matchLive).toHaveBeenCalledWith({ url: POSTING_URL, html: PAGE_HTML });
    expect(res).toEqual({ ok: true, kind: 'matchLive', result });
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    paired();
    scriptResults(PAGE_HTML);
    const result = {
      ok: false,
      error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
    };
    mockClient.matchLive.mockResolvedValue(result);

    expect(await send({ kind: 'matchLive' })).toEqual({ ok: true, kind: 'matchLive', result });
  });

  it('surfaces a fixed capture-failure message when the page DOM could not be captured — no URL-mode fallback', async () => {
    paired();
    // Non-string / empty result → captureTabHtml throws.
    scriptResults(null);

    expect(await send({ kind: 'matchLive' })).toEqual(RELOAD_HINT);
    expect(mockClient.matchLive).not.toHaveBeenCalled();
  });

  it('surfaces a transport-level rejection as ok:false', async () => {
    paired();
    scriptResults(PAGE_HTML);
    mockClient.matchLive.mockRejectedValue(new Error(DESKTOP_DOWN));

    expect(await send({ kind: 'matchLive' })).toEqual({ ok: false, error: DESKTOP_DOWN });
  });
});

describe('matchLive → on-page fit badge injection', () => {
  it('injects the badge when getShowFitBadge is on, including the applied chip and salary facts', async () => {
    await setShowFitBadge(true);
    paired();
    scriptResults(PAGE_HTML); // content.js capture
    mockClient.matchLive.mockResolvedValue(
      matchOk({
        gaps: ['kubernetes'],
        salary: { posting: '€70,000–€90,000', expectation: '€80,000' },
      })
    );
    mockClient.checkApplied.mockResolvedValue({ found: true, status: 'saved' });
    scriptResults(undefined, undefined); // fit-badge.js files, then its func call

    await send({ kind: 'matchLive' });
    await flush();

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['fit-badge.js'] })
    );
    const funcCall = fitBadgeRenderCall();
    expect(funcCall).toBeDefined();
    const view = funcCall.args[0] as FitBadgeView;
    expect(view.score).toBe(82);
    expect(view.band).toBe('strong match');
    expect(view.applied).toBe('saved');
    expect(view.salary).toEqual({ posting: '€70,000–€90,000', expectation: '€80,000' });
  });

  it('never injects the badge when getShowFitBadge is off (the default)', async () => {
    paired();
    scriptResults(PAGE_HTML);
    mockClient.matchLive.mockResolvedValue(matchOk());

    await send({ kind: 'matchLive' });
    await flush();

    expect(executeScriptMock).not.toHaveBeenCalledWith(badgeInjected());
  });

  it('never affects the popup response even when badge injection fails', async () => {
    await setShowFitBadge(true);
    paired();
    scriptResults(PAGE_HTML);
    mockClient.matchLive.mockResolvedValue(matchOk());
    mockClient.checkApplied.mockRejectedValue(new Error('boom'));
    executeScriptMock.mockRejectedValueOnce(new Error('injection blocked'));

    const res = await send({ kind: 'matchLive' });
    await flush();

    expect(res.ok).toBe(true);
    expect(res).toMatchObject({ kind: 'matchLive', result: { ok: true, combined: 82 } });
  });

  // Both abort the BADGE silently — the popup's own response is unaffected.
  it.each([
    [
      'binds the badge to the tab resolved BEFORE the desktop round trip and aborts it silently when a different tab is active afterwards (tab switch mid-request)',
      ['https://other.example.com/', 9],
    ],
    [
      'aborts the badge silently when the SAME tab navigated to a different url during the round trip (same-tab navigation)',
      ['https://jobs.example.com/posting/10', 7],
    ],
  ] as const)('%s', async (_label, [reverified, reverifiedId]) => {
    await setShowFitBadge(true);
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    // Resolved once up front with the url, then re-verified right before injection.
    tabSequence([POSTING_URL], [reverified, reverifiedId]);
    scriptResults(PAGE_HTML);
    mockClient.matchLive.mockResolvedValue(matchOk({ gaps: ['kubernetes'] }));

    const res = await send({ kind: 'matchLive' });
    await flush();

    expect(res).toMatchObject({ kind: 'matchLive', result: { ok: true, combined: 82 } });
    // The captured/sent url is the tab resolved BEFORE the switch, not the new one.
    expect(mockClient.matchLive).toHaveBeenCalledWith({ url: POSTING_URL, html: PAGE_HTML });
    expect(mockClient.checkApplied).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalledWith(badgeInjected());
  });

  // PR review finding: the tab-and-url re-check above runs BEFORE
  // maybeShowFitBadge's own later awaits (getShowFitBadge, checkApplied), so a
  // navigation during either isn't caught by it. The injected `func` re-checks
  // `location.href` against the captured url as the LAST possible step, IN the
  // page. These drive that `func` directly (extracted from the mocked
  // `executeScript` call).
  describe('the final in-page url check inside the injected renderer', () => {
    afterEach(() => {
      vi.unstubAllGlobals();
    });

    async function driveMatchLiveAndRender(
      liveHref: string
    ): Promise<{ runnerSpy: ReturnType<typeof vi.fn>; args: unknown[] }> {
      await setShowFitBadge(true);
      paired();
      scriptResults(PAGE_HTML, undefined, undefined); // capture, fit-badge.js files, func
      mockClient.matchLive.mockResolvedValue(matchOk());
      mockClient.checkApplied.mockResolvedValue({ found: false });
      await send({ kind: 'matchLive' });
      await flush();

      const { func, args } = fitBadgeRenderCall();
      // The url captured before the round trip is threaded through as the
      // renderer's third arg, unchanged.
      expect(args[2]).toBe(POSTING_URL);
      const runnerSpy = vi.fn();
      vi.stubGlobal('__ajhRenderFitBadge', runnerSpy);
      vi.stubGlobal('location', { href: liveHref } as Location);
      func(...args);
      return { runnerSpy, args };
    }

    it('still renders when the live page url matches the captured url (happy path)', async () => {
      const { runnerSpy, args } = await driveMatchLiveAndRender(POSTING_URL);

      // Issue #1221: the runner is handed the captured url as its second arg —
      // the badge then keeps watching `location.href` against it after render.
      expect(runnerSpy).toHaveBeenCalledWith(args[0], args[2]);
    });

    it.each([
      [
        'does not render after a full navigation between the desktop reply and the injection (a fresh document with a different url)',
        'https://jobs.example.com/posting/999',
      ],
      [
        // Same origin/document, e.g. a client-side route change — still a
        // different posting, so it must not match.
        'does not render after an SPA-style url change with the fit-badge global already installed (same document, different url)',
        `${POSTING_URL}?ref=nav`,
      ],
    ])('%s', async (_label, liveHref) => {
      const { runnerSpy } = await driveMatchLiveAndRender(liveHref);

      expect(runnerSpy).not.toHaveBeenCalled();
    });
  });
});
