/**
 * #1227: the popup's own first-time Fill confirmation. The popup uses the SAME
 * first-time-per-site inset the panel does (one shared remembered-host set,
 * `fillConfirmDontAskHosts`), hosted in `#popup-fill-confirm-host` inside
 * #view-import so it only exists while connected. The origin is resolved ON
 * DEMAND at Fill-click time from the `activeTab`-granted tab url (never from a
 * stored answer-state push — a push can be absent or stale, and round-1's
 * `null` default silently approved the first Fill); an unreadable url refuses
 * visibly instead. The capture → confirm → re-validate pattern is the panel's own.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', async () => (await import('./browser-mock')).popupBrowserMock());
vi.mock('../../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

import { bootPopup, byId, flush, statusPusher } from './test-support';

const { bootstrapNotice } = await bootPopup();
const push = statusPusher();
const sendMessageMock = vi.mocked(browser.runtime.sendMessage);

const confirmHost = byId<HTMLElement>('popup-fill-confirm-host');
const fillBtn = byId<HTMLButtonElement>('btn-fill');
const inset = () => confirmHost.querySelector<HTMLElement>('.inset');
const importMsg = () => byId<HTMLElement>('import-msg');
const FILL = { kind: 'fill' };

/** The active-tab url the popup faces once the toolbar-click grant makes it
 *  readable — the shape `tabs.query` returns under a live `activeTab`
 *  grant (mirrors `background.ts`'s `activeTabOriginAtGesture` read). */
const GRANTED_URL = 'https://example.com/jobs/123';
const grantActiveTab = (url: string = GRANTED_URL): void => {
  vi.mocked(browser.tabs.query).mockResolvedValue([{ id: 7, url }]);
};

beforeEach(async () => {
  // Route the popup's own traffic by kind: everything the auto-checks fire
  // answers as a benign not-found, `getStatus` as a normal connected status,
  // and `fill` as a success — so a fill that DOES go out is always
  // attributable to `{ kind: 'fill' }` in the assert below.
  sendMessageMock.mockReset();
  sendMessageMock.mockImplementation(async (req: unknown) => {
    const kind = (req as { kind?: string }).kind;
    if (kind === 'fill') {
      return {
        ok: true,
        kind: 'fill',
        summary: {
          filled: [{ field: 'name', value: 'Ada', count: 1 }],
          nameSplit: null,
          filledNothing: false,
        },
      };
    }
    if (kind === 'getStatus') {
      return {
        ok: true,
        kind: 'status',
        status: { phase: 'connected', port: null, hasToken: true },
      };
    }
    return { ok: true, kind: 'appliedCheck', result: { found: false } };
  });
  vi.mocked(browser.storage.local.get).mockReset().mockResolvedValue({});
  vi.mocked(browser.storage.local.set).mockReset().mockResolvedValue(undefined);
  // Default to the REDACTED shape — a tab with no readable url, exactly
  // what `tabs.query` returns without a grant. The granted-flow tests each
  // opt into a url-bearing tab via `grantActiveTab`, mirroring what an
  // actual toolbar-click `activeTab` grant makes readable.
  vi.mocked(browser.tabs.query)
    .mockReset()
    .mockResolvedValue([{ id: 7 }]);
  // Force a genuine non-connected → connected transition (the auto-checks
  // only fire on ENTERING connected).
  push('searching');
  push('connected');
  await flush();
  fillBtn.disabled = false;
  // A previous test may have left the first-time inset OPEN and unanswered
  // (the user's "Not now" never came) — close it through the real UI so
  // this test starts from the true initial state, not a leaked one.
  inset()?.querySelector<HTMLButtonElement>('button.btn--quiet')?.click();
  await flush();
  await bootstrapNotice();
});

describe('popup Fill first-time confirmation (#1227)', () => {
  it('never fills without the first-time confirmation when no answer-state push has arrived', async () => {
    // The round-1 regression this guards: with NO pushed origin,
    // `currentOrigin` stayed `null`, `hostOf(null)` early-approved the
    // confirmation, and the re-check `null === null` sent `fill` with
    // nothing on screen. The fix resolves the origin from the granted tab
    // url INSTEAD of the push — so the first click must show the inset and
    // go nowhere until the user answers it.
    grantActiveTab();

    fillBtn.click();
    await flush();

    // The user SAW the confirmation (the inset), and no fill went out.
    expect(inset()).not.toBeNull();
    expect(inset()?.hidden).toBe(false);
    expect(sendMessageMock).not.toHaveBeenCalledWith(FILL);
  });

  it('refuses visibly — and never fills — when the tab url cannot be read (no activeTab grant)', async () => {
    // REDACTED shape (the beforeEach default `{ id: 7 }`, no url): the old
    // flow approved this — `hostOf(null)` early-returned without ever
    // showing anything. The fix must surface a VISIBLE refusal on the shared
    // status line, never a silent fill.
    fillBtn.click();
    await flush();
    await flush();

    expect(sendMessageMock).not.toHaveBeenCalledWith(FILL);
    expect(inset()?.hidden).toBe(true);
    expect(importMsg().hidden).toBe(false);
    expect(importMsg().textContent).toContain('Could not read this page');
  });

  it('asks before the FIRST Fill, remembers the host when the checkbox is ticked, and only then fills', async () => {
    grantActiveTab();

    fillBtn.click();
    await flush();

    // The confirmation is up, inside #view-import's own fixed host — and no
    // fill has gone out while it is unanswered.
    expect(confirmHost.parentElement?.id).toBe('view-import');
    expect(inset()).not.toBeNull();
    expect(inset()?.hidden).toBe(false);
    expect(sendMessageMock).not.toHaveBeenCalledWith(FILL);

    // "Don't ask again" + confirm → the host lands in the SAME shared set the
    // panel reads/writes, and the fill fires.
    const checkbox = inset()?.querySelector<HTMLInputElement>('input[type="checkbox"]');
    if (!checkbox) throw new Error('confirmation checkbox missing');
    checkbox.checked = true;
    inset()?.querySelector<HTMLButtonElement>('button.btn--primary')?.click();

    await vi.waitFor(() =>
      expect(vi.mocked(browser.storage.local.set)).toHaveBeenCalledWith({
        fillConfirmDontAskHosts: ['example.com'],
      })
    );
    await vi.waitFor(() => expect(sendMessageMock).toHaveBeenCalledWith(FILL));
  });

  it('skips the confirmation entirely when the host is already in the shared remembered set', async () => {
    // Pre-seeded by the PANEL's own remember (or a previous popup session) —
    // the same `fillConfirmDontAskHosts` key, so the popup never double-asks.
    vi.mocked(browser.storage.local.get).mockResolvedValueOnce({
      fillConfirmDontAskHosts: ['example.com'],
    });
    grantActiveTab();

    fillBtn.click();

    await vi.waitFor(() => expect(sendMessageMock).toHaveBeenCalledWith(FILL));
    expect(inset()?.hidden).toBe(true);
  });

  it('never sends fill while the confirmation stays unanswered (the popup closes mid-confirm)', async () => {
    grantActiveTab();

    fillBtn.click();
    await flush();

    expect(inset()?.hidden).toBe(false);
    // No answer — the popup can close at any moment; fill must not have fired.
    expect(sendMessageMock).not.toHaveBeenCalledWith(FILL);
    await flush();
    expect(sendMessageMock).not.toHaveBeenCalledWith(FILL);
  });

  it('aborts the fill when the page origin changes while the confirmation is open', async () => {
    grantActiveTab('https://example.com/jobs/1');

    fillBtn.click();
    await flush();
    expect(inset()?.hidden).toBe(false);

    // The page navigates while the user is deciding — the re-resolve at
    // confirm time reads a DIFFERENT url from the (still granted) tab.
    grantActiveTab('https://other.example.com/jobs/2');

    inset()?.querySelector<HTMLButtonElement>('button.btn--primary')?.click();

    // Re-validation: the approval belonged to example.com, never other.com.
    await flush();
    await flush();
    expect(sendMessageMock).not.toHaveBeenCalledWith(FILL);
  });
});
