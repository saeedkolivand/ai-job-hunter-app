/** Thin handlers that ask the desktop about the active job / the extension's settings. */

import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { PopupRequest } from '../lib/messages';
import {
  activeTab,
  browser,
  DESKTOP_DOWN,
  mockClient,
  POSTING_URL,
  resetMocks,
  send,
  tabsQueryMock,
} from './test-support';

beforeEach(resetMocks);

const SETTINGS = { autofill: true, aiAssist: false, autotrack: false };

describe('appliedCheck request — always ok:true, every failure folds into found:false', () => {
  it('returns the checkApplied result on success', async () => {
    activeTab();
    const result = {
      found: true,
      applicationId: 'app-1',
      status: 'applied',
      appliedAt: 1_718_000_000_000,
    };
    mockClient.checkApplied.mockResolvedValue(result);

    const res = await send({ kind: 'appliedCheck' });

    expect(res).toEqual({ ok: true, kind: 'appliedCheck', result });
    expect(mockClient.checkApplied).toHaveBeenCalledWith(POSTING_URL);
  });

  it('folds a checkApplied REJECTION (e.g. old-desktop unknown message type) into found:false, never ok:false', async () => {
    activeTab();
    mockClient.checkApplied.mockRejectedValue(
      new Error("The desktop app sent an error: unknown message type 'applied.check'")
    );

    const res = await send({ kind: 'appliedCheck' });

    expect(res).toEqual({ ok: true, kind: 'appliedCheck', result: { found: false } });
  });

  it('folds a missing active tab into found:false, never ok:false', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'appliedCheck' });

    expect(res).toEqual({ ok: true, kind: 'appliedCheck', result: { found: false } });
    expect(mockClient.checkApplied).not.toHaveBeenCalled();
  });
});

describe('trustLineJob request — always ok:true, every failure folds into title:null/company:null', () => {
  const folded = { ok: true, kind: 'trustLineJob', title: null, company: null };

  it('returns title/company from a successful agent.query', async () => {
    activeTab();
    mockClient.agentQuery.mockResolvedValue({
      ok: true,
      resource: 'job',
      data: { title: 'Senior Rust Engineer', company: 'Acme' },
    });

    const res = await send({ kind: 'trustLineJob' });

    expect(res).toEqual({
      ok: true,
      kind: 'trustLineJob',
      title: 'Senior Rust Engineer',
      company: 'Acme',
    });
    expect(mockClient.agentQuery).toHaveBeenCalledWith('job', { url: POSTING_URL });
  });

  it.each([
    [
      'a desktop refusal (e.g. Autofill off)',
      () =>
        mockClient.agentQuery.mockResolvedValue({
          ok: false,
          resource: 'job',
          error: 'Assisted autofill is off.',
        }),
    ],
    [
      'a rejection (no connection / throttled)',
      () => mockClient.agentQuery.mockRejectedValue(new Error('Desktop app not reachable.')),
    ],
    [
      'a malformed data shape (missing title/company)',
      () => mockClient.agentQuery.mockResolvedValue({ ok: true, resource: 'job', data: {} }),
    ],
  ])('folds %s into title:null/company:null, never ok:false', async (_label, arrange) => {
    activeTab();
    arrange();

    expect(await send({ kind: 'trustLineJob' })).toEqual(folded);
  });

  it('folds a missing active tab into title:null/company:null', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'trustLineJob' });

    expect(res).toEqual(folded);
    expect(mockClient.agentQuery).not.toHaveBeenCalled();
  });

  it.each([
    [
      'strips the desktop fence wrapper off title/company (#1229 — the render layer must never show `<job_posting>…</job_posting>`)',
      {
        title: '<job_posting>\nSenior Rust Engineer\n</job_posting>',
        company: '<job_posting>\nAcme\n</job_posting>',
      },
      { title: 'Senior Rust Engineer', company: 'Acme' },
    ],
    [
      'degrades an EMPTY fenced value to null via the strip-then-trim guard, never the raw wrapper',
      { title: '<job_posting>\n\n</job_posting>', company: 'Acme' },
      { title: null, company: 'Acme' },
    ],
    [
      'leaves a title that merely contains angle brackets alone — only the EXACT fence wrapper is stripped',
      { title: '<b>Senior</b> Rust Engineer', company: 'Acme & Sons' },
      { title: '<b>Senior</b> Rust Engineer', company: 'Acme & Sons' },
    ],
  ])('%s', async (_label, data, expected) => {
    activeTab();
    mockClient.agentQuery.mockResolvedValue({ ok: true, resource: 'job', data });

    expect(await send({ kind: 'trustLineJob' })).toEqual({
      ok: true,
      kind: 'trustLineJob',
      ...expected,
    });
  });
});

// Settings errors are NOT folded, mirroring statusUpdate's discipline.
describe('settingsGet / settingsSet requests (PR1 — extension read tier)', () => {
  it('settingsGet returns the settingsGet result on success', async () => {
    mockClient.settingsGet.mockResolvedValue({ ok: true, settings: SETTINGS });

    const res = await send({ kind: 'settingsGet' });

    expect(res).toEqual({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: SETTINGS },
    });
  });

  it('settingsSet sends the key/enabled and returns the new settings on success', async () => {
    mockClient.settingsSet.mockResolvedValue({ ok: true, settings: SETTINGS });

    const res = await send({ kind: 'settingsSet', key: 'autofill', enabled: true });

    expect(res).toEqual({
      ok: true,
      kind: 'settingsSet',
      result: { ok: true, settings: SETTINGS },
    });
    expect(mockClient.settingsSet).toHaveBeenCalledWith('autofill', true);
  });

  it.each([
    ['settingsGet', { kind: 'settingsGet' } as PopupRequest, mockClient.settingsGet, 'not paired'],
    [
      'settingsSet',
      { kind: 'settingsSet', key: 'autofill', enabled: true } as PopupRequest,
      mockClient.settingsSet,
      'invalid_settings_request',
    ],
  ])(
    '%s passes a desktop-side refusal straight through as result, never folds it',
    async (kind, req, mock, error) => {
      mock.mockResolvedValue({ ok: false, error });

      expect(await send(req)).toEqual({ ok: true, kind, result: { ok: false, error } });
    }
  );
});

describe('documentsList / prepGet requests (PR2, PR4)', () => {
  it.each([
    ['documentsList', 'documents', { generation: null, documents: [] }],
    ['prepGet', 'prep', { generation: null }],
  ] as const)(
    '%s resolves the active tab url server-side, calls agent.query(%s, {url}), and echoes the url back',
    async (kind, resource, data) => {
      activeTab();
      mockClient.agentQuery.mockResolvedValue({ ok: true, resource, data });

      const res = await send({ kind });

      expect(mockClient.agentQuery).toHaveBeenCalledWith(resource, { url: POSTING_URL });
      expect(res).toEqual({
        ok: true,
        kind,
        result: { ok: true, resource, data },
        url: POSTING_URL,
      });
    }
  );

  it.each([
    ['documentsList', 'documents'],
    ['prepGet', 'prep'],
  ] as const)(
    '%s passes a desktop-side refusal straight through as result, never folds it (unlike trustLineJob)',
    async (kind, resource) => {
      tabsQueryMock.mockResolvedValue([]);
      const refusal = { ok: false, resource, error: 'Assisted autofill is off.' };
      mockClient.agentQuery.mockResolvedValue(refusal);

      expect(await send({ kind })).toEqual({ ok: true, kind, result: refusal, url: '' });
    }
  );
});

describe('assistCancel request (PR4)', () => {
  it('calls BridgeClient.cancelCurrent and always answers ok:true', async () => {
    const res = await send({ kind: 'assistCancel' });

    expect(mockClient.cancelCurrent).toHaveBeenCalled();
    expect(res).toEqual({ ok: true, kind: 'assistCancel' });
  });
});

describe('autoSaveNotice request (PR4)', () => {
  it('returns null when no notice is pending', async () => {
    expect(await send({ kind: 'autoSaveNotice' })).toEqual({
      ok: true,
      kind: 'autoSaveNotice',
      text: null,
    });
  });

  it('returns and clears a pending notice — read-once', async () => {
    const text = 'Saved 1 answer from this submit.';
    activeTab(undefined, 7);
    await browser.storage.session.set({ autoSaveNotice: { text, tabId: 7 } });

    expect(await send({ kind: 'autoSaveNotice' })).toEqual({
      ok: true,
      kind: 'autoSaveNotice',
      text,
    });
    expect(await send({ kind: 'autoSaveNotice' })).toEqual({
      ok: true,
      kind: 'autoSaveNotice',
      text: null,
    });
  });
});

describe('autoSaveNotice request — per-tab (#1369)', () => {
  it('resolves the tab in the requesting window (windowId pass-through)', async () => {
    activeTab(undefined, 7);
    await send({ kind: 'autoSaveNotice', windowId: 55 });
    expect(tabsQueryMock).toHaveBeenCalledWith({ active: true, windowId: 55 });
  });
  it("leaves another tab's notice pending for its own tab", async () => {
    await browser.storage.session.set({ autoSaveNotice: { text: 'Saved 1 answer.', tabId: 7 } });
    activeTab(undefined, 9);
    expect(await send({ kind: 'autoSaveNotice' })).toEqual({
      ok: true,
      kind: 'autoSaveNotice',
      text: null,
    });
    activeTab(undefined, 7);
    expect(await send({ kind: 'autoSaveNotice' })).toMatchObject({ text: 'Saved 1 answer.' });
  });
});

describe('statusUpdate request — errors are NOT folded (unlike appliedCheck)', () => {
  it('returns the updateStatus success result', async () => {
    activeTab();
    const result = { ok: true, applicationId: 'app-1', status: 'applied' };
    mockClient.updateStatus.mockResolvedValue(result);

    const res = await send({ kind: 'statusUpdate' });

    expect(res).toEqual({ ok: true, kind: 'statusUpdate', result });
    expect(mockClient.updateStatus).toHaveBeenCalledWith(POSTING_URL);
  });

  it('pushes jobStatusChanged for the active url on success so an open side panel refreshes (#1410)', async () => {
    activeTab();
    vi.mocked(browser.runtime.sendMessage).mockClear();
    mockClient.updateStatus.mockResolvedValue({
      ok: true,
      applicationId: 'app-1',
      status: 'applied',
    });

    await send({ kind: 'statusUpdate' });

    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({
      ok: true,
      kind: 'jobStatusChanged',
      url: POSTING_URL,
    });
  });

  it('does NOT push jobStatusChanged when the desktop refused the flip (#1410)', async () => {
    activeTab();
    vi.mocked(browser.runtime.sendMessage).mockClear();
    mockClient.updateStatus.mockResolvedValue({ ok: false, error: 'nope' });

    await send({ kind: 'statusUpdate' });

    expect(browser.runtime.sendMessage).not.toHaveBeenCalled();
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    activeTab('https://jobs.example.com/posting/none');
    const result = { ok: false, error: "couldn't find a saved job for this page" };
    mockClient.updateStatus.mockResolvedValue(result);

    expect(await send({ kind: 'statusUpdate' })).toEqual({
      ok: true,
      kind: 'statusUpdate',
      result,
    });
  });

  it('surfaces a transport-level rejection as ok:false at the OUTER level (UNLIKE appliedCheck, which folds every rejection)', async () => {
    activeTab();
    mockClient.updateStatus.mockRejectedValue(new Error(DESKTOP_DOWN));

    expect(await send({ kind: 'statusUpdate' })).toEqual({ ok: false, error: DESKTOP_DOWN });
  });

  it('surfaces "Could not read the current tab URL." when there is no active tab, without calling updateStatus', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'statusUpdate' });

    expect(res).toEqual({ ok: false, error: 'Could not read the current tab URL.' });
    expect(mockClient.updateStatus).not.toHaveBeenCalled();
  });
});
