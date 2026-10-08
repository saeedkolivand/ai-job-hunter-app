/** Auto-track (Task #22): `submitDetected` routing and arming the submit watcher after a gesture. */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { PopupResponse } from '../lib/messages';
import { SUBMIT_DETECTED_MSG } from '../lib/submit-watch';
import {
  backgroundModule,
  browser,
  EMAIL_SUMMARY,
  executeScriptMock,
  EXTENSION_ID,
  flush,
  listener,
  mockClient,
  paired,
  resetMocks,
  scriptResults,
  send,
  setStampResultsPages,
} from './test-support';

beforeEach(resetMocks);

const armed = () => expect.objectContaining({ files: ['submit-watch.js'] });
/** Hand the raw listener a `submitDetected` message from `senderId`. */
const submitDetected = (senderId: string) =>
  listener?.({ kind: SUBMIT_DETECTED_MSG, url: 'https://jobs.example.com/posting/9' }, {
    id: senderId,
  } as never);

describe('SUBMIT_DETECTED_MSG parity (Task #22 review closure)', () => {
  it('the background/guards.ts literal matches the imported lib/submit-watch.ts const — a future edit to one side cannot silently break routing', () => {
    expect(backgroundModule.SUBMIT_DETECTED_MSG).toBe(SUBMIT_DETECTED_MSG);
  });
});

describe('submitDetected message — not a popup request (Task #22 review closure)', () => {
  it('returns undefined (no popup response channel) and routes to handleSubmitDetected, which auto-marks a tracked saved job applied when the opt-in is ON', async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
    mockClient.checkApplied.mockResolvedValue({ found: true, status: 'saved' });
    mockClient.updateStatus.mockResolvedValue({
      ok: true,
      applicationId: 'app-1',
      status: 'applied',
    });

    expect(submitDetected(EXTENSION_ID)).toBeUndefined();
    await flush();

    expect(mockClient.checkApplied).toHaveBeenCalledWith('https://jobs.example.com/posting/9');
    expect(mockClient.updateStatus).toHaveBeenCalledWith(
      'https://jobs.example.com/posting/9',
      true
    );
    // The confirmed flip is PUSHED to the side panel over the shared channel —
    // exactly one broadcast, carrying the flipped application's url (#1233).
    const pushes = vi
      .mocked(browser.runtime.sendMessage)
      .mock.calls.map((call) => call[0] as PopupResponse)
      .filter((m) => m.ok && m.kind === 'jobStatusChanged');
    expect(pushes).toEqual([
      { ok: true, kind: 'jobStatusChanged', url: 'https://jobs.example.com/posting/9' },
    ]);
  });

  it('is ignored when the sender is not this extension (belt-and-braces MV3 hygiene)', async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
    mockClient.checkApplied.mockResolvedValue({ found: true, status: 'saved' });

    expect(submitDetected('some-other-extension-id')).toBeUndefined();
    await flush();

    expect(mockClient.checkApplied).not.toHaveBeenCalled();
    expect(mockClient.updateStatus).not.toHaveBeenCalled();
  });
});

describe('submitDetected with a malformed answers payload is not routed to auto-track', () => {
  it.each([
    ['a string', 'x'],
    ['entries missing the answer text', [{ question: 1 }]],
  ])(
    'answers as %s from our own sender never reaches handleSubmitDetected',
    async (_label, answers) => {
      mockClient.autotrackEnabled.mockResolvedValue(true);
      mockClient.checkApplied.mockResolvedValue({ found: true, status: 'saved' });

      listener?.(
        { kind: SUBMIT_DETECTED_MSG, url: 'https://jobs.example.com/posting/9', answers },
        { id: EXTENSION_ID } as never,
        () => {}
      );
      await flush();

      expect(mockClient.autotrackEnabled).not.toHaveBeenCalled();
      expect(mockClient.checkApplied).not.toHaveBeenCalled();
      expect(mockClient.saveAnswers).not.toHaveBeenCalled();
    }
  );
});

describe('auto-save notice is recorded for the submitting tab (#1369)', () => {
  it("stores the sender tab's id with the notice", async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
    mockClient.checkApplied.mockResolvedValue({ found: false });
    mockClient.saveAnswers.mockResolvedValue({ ok: true, saved: 1 });

    listener?.(
      {
        kind: SUBMIT_DETECTED_MSG,
        url: 'https://jobs.example.com/posting/9',
        answers: [{ question: 'Why us?', answer: 'Because.' }],
      },
      { id: EXTENSION_ID, tab: { id: 42 } } as never,
      () => {}
    );
    await flush();

    const stored = await browser.storage.session.get('autoSaveNotice');
    expect(stored.autoSaveNotice).toMatchObject({ tabId: 42 });
  });
});

describe('the fit badge "Open the panel" message', () => {
  const click = (sender: object) =>
    listener?.({ kind: 'ajhOpenPanelFromBadge' }, sender as never, () => {});

  it('opens the panel for our own sender with a tab', () => {
    expect(click({ id: EXTENSION_ID, tab: { id: 5 } })).toBeUndefined();
    expect(browser.sidePanel.open).toHaveBeenCalledWith({ tabId: 5 });
  });

  it.each([
    ['a foreign sender', { id: 'some-other-extension-id', tab: { id: 5 } }],
    ['our own sender without a tab', { id: EXTENSION_ID }],
  ])('does not open the panel for %s', (_label, sender) => {
    vi.mocked(browser.sidePanel.open).mockClear();

    expect(click(sender)).toBeUndefined();
    expect(browser.sidePanel.open).not.toHaveBeenCalled();
  });
});

describe('arming the submit watcher after a gesture request (Task #22 review closure)', () => {
  beforeEach(() => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
  });

  it('a successful GESTURE_KINDS request (e.g. fill) injects submit-watch.js when the opt-in is ON', async () => {
    paired('https://example.com/apply');
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    scriptResults(undefined, EMAIL_SUMMARY); // fill.js registration, then its call

    await send({ kind: 'fill' });
    await flush(); // the arm is fire-and-forget — flush it before asserting

    expect(mockClient.autotrackEnabled).toHaveBeenCalled();
    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 7 },
      files: ['submit-watch.js'],
    });
    // Second step: the arm func gets captureAnswers first and the global key LAST.
    const armCall = executeScriptMock.mock.calls.find(
      (c) => (c[0] as { args?: unknown[] }).args?.[1] === '__ajhArmSubmitWatch'
    );
    expect((armCall?.[0] as { args: unknown[] }).args).toEqual([false, '__ajhArmSubmitWatch']);
  });

  it('a non-gesture request (getStatus) never arms the watcher', async () => {
    await send({ kind: 'getStatus' });
    await flush();

    expect(mockClient.autotrackEnabled).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalledWith(armed());
  });

  it('a fieldsProbe request never arms the watcher (a passive scan, not a user gesture)', async () => {
    paired('https://example.com/apply');
    scriptResults({ hasFormFields: true, hasAnswerFields: true });

    await send({ kind: 'fieldsProbe' });
    await flush();

    expect(mockClient.autotrackEnabled).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalledWith(armed());
  });

  it('a successful stampResults request never arms the watcher — it is read-only (no form interaction), so arming it would let a later, unrelated submit-like interaction on a results page auto-mark a saved application applied (PR review finding)', async () => {
    paired('https://x/jobs?q=data');
    await setStampResultsPages(true);
    scriptResults(undefined, []); // results-stamp.js files, then the collect func — no cards

    const res = await send({ kind: 'stampResults' });
    await flush();

    expect(res.ok).toBe(true);
    expect(mockClient.autotrackEnabled).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalledWith(armed());
  });
});
