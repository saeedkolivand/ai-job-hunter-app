/**
 * Unit tests for the Settings page controller: render, the theme segmented
 * control persisting + applying `data-theme`, the sites list (empty state +
 * Forget), and the LIVE "What the extension may do" toggles (PR1, R7 —
 * three switches today, the fourth lands in PR4; see options.ts's own doc).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: {
      sendMessage: vi.fn(async () => ({ ok: false, error: 'not configured' })),
      onMessage: { addListener: vi.fn() },
      getManifest: vi.fn(() => ({ version: '1.2.3' })),
    },
    tabs: { create: vi.fn(async () => undefined) },
    storage: {
      local: { get: vi.fn(async () => ({})), set: vi.fn(), remove: vi.fn() },
    },
  },
}));

vi.mock('../connection-status/connection-status', () => ({
  mountConnectionStatus: vi.fn(() => ({ start: vi.fn() })),
  PAIRING_DEEP_LINK: 'ajh://settings/extension',
}));

vi.mock('../lib/site-memory', () => ({
  getRememberedHosts: vi.fn(async () => [] as string[]),
  forgetHost: vi.fn(async () => undefined),
}));

function buildOptionsDom(): void {
  document.body.innerHTML = `
    <div id="connection-pill-host"></div>
    <div id="connection-views-host"></div>
    <button id="btn-unpair" hidden></button>
    <button id="btn-open-app-settings"></button>
    <div id="sites-list"></div>
    <div id="permissions-list"></div>
    <p id="permissions-error" class="hint" role="status" hidden></p>
    <div id="theme-seg">
      <button type="button" data-theme-choice="system">System</button>
      <button type="button" data-theme-choice="light">Light</button>
      <button type="button" data-theme-choice="dark">Dark</button>
    </div>
    <div id="default-tab-seg">
      <button type="button" data-tab-choice="job">Job</button>
      <button type="button" data-tab-choice="answers">Answers</button>
    </div>
    <div class="set-row toggle-row">
      <p id="title-fit-badge">Show the on-page fit badge after Check fit</p>
      <button id="toggle-fit-badge" role="switch" aria-checked="false" aria-labelledby="title-fit-badge"></button>
    </div>
    <div class="set-row toggle-row">
      <p id="title-stamp-results">Stamp saved/applied on results pages</p>
      <button id="toggle-stamp-results" role="switch" aria-checked="false" aria-labelledby="title-stamp-results"></button>
    </div>
    <div id="shortcuts-list"></div>
    <p id="about-version"></p>
    <a id="link-privacy" href="#"></a>
    <a id="link-help" href="#"></a>
    <a id="link-source" href="#"></a>
    <a id="link-report" href="#"></a>
  `;
}

buildOptionsDom();

// Vitest 5 clears every mock before each test by default, which would wipe
// the module-load-time `mountConnectionStatus(...)` call this file asserts
// on — opt out, same rationale as sidepanel.test.ts's identical guard.
vi.setConfig({ clearMocks: false });

await import('./options');

const { renderSites } = await import('./options');

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));
const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

describe('render', () => {
  it('mounts connection-status against the pill + views hosts', async () => {
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    expect(vi.mocked(mountConnectionStatus)).toHaveBeenCalledWith(
      byId('connection-pill-host'),
      byId('connection-views-host'),
      expect.objectContaining({ send: expect.any(Function), onStatus: expect.any(Function) })
    );
  });

  it('shows the version from the manifest', () => {
    expect(byId('about-version').textContent).toContain('1.2.3');
  });

  it('sets the privacy/source/report links to the real repo, not a placeholder', () => {
    expect(byId<HTMLAnchorElement>('link-privacy').href).toBe('https://aijobhunter.app/privacy');
    expect(byId<HTMLAnchorElement>('link-source').href).toBe(
      'https://github.com/saeedkolivand/ai-job-hunter-app'
    );
  });
});

describe('theme segmented control', () => {
  it('applies data-theme and marks the clicked choice active', async () => {
    document.querySelector<HTMLButtonElement>('[data-theme-choice="dark"]')!.click();
    await flush();

    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(document.querySelector('[data-theme-choice="dark"]')!.classList.contains('active')).toBe(
      true
    );
    expect(
      document.querySelector('[data-theme-choice="system"]')!.classList.contains('active')
    ).toBe(false);
  });

  it('persists the choice to storage.local', () => {
    document.querySelector<HTMLButtonElement>('[data-theme-choice="light"]')!.click();
    expect(browser.storage.local.set).toHaveBeenCalledWith({ theme: 'light' });
  });
});

describe('default panel tab segmented control', () => {
  it('persists the choice and marks it active', () => {
    document.querySelector<HTMLButtonElement>('[data-tab-choice="answers"]')!.click();

    expect(browser.storage.local.set).toHaveBeenCalledWith({ defaultPanelTab: 'answers' });
    expect(
      document.querySelector('[data-tab-choice="answers"]')!.classList.contains('active')
    ).toBe(true);
  });
});

describe('appearance toggles', () => {
  it('toggles the fit-badge switch on click and persists it', () => {
    const btn = byId<HTMLButtonElement>('toggle-fit-badge');
    expect(btn.classList.contains('on')).toBe(false);

    btn.click();

    expect(btn.classList.contains('on')).toBe(true);
    expect(btn.getAttribute('aria-checked')).toBe('true');
    expect(browser.storage.local.set).toHaveBeenCalledWith({ showFitBadge: true });
  });

  it('has an accessible name (aria-labelledby the row title)', () => {
    const btn = byId<HTMLButtonElement>('toggle-fit-badge');
    const labelledbyId = btn.getAttribute('aria-labelledby');
    expect(labelledbyId).toBeTruthy();
    expect(document.getElementById(labelledbyId!)?.textContent).toBe(
      'Show the on-page fit badge after Check fit'
    );
  });

  it('shows both rows — PR3 wired the on-page badge/stamps these preferences gate', () => {
    expect(byId('toggle-fit-badge').closest('.toggle-row')).toHaveProperty('hidden', false);
    expect(byId('toggle-stamp-results').closest('.toggle-row')).toHaveProperty('hidden', false);
  });

  it('toggles the stamp-results switch on click and persists it', () => {
    const btn = byId<HTMLButtonElement>('toggle-stamp-results');
    expect(btn.classList.contains('on')).toBe(false);

    btn.click();

    expect(btn.classList.contains('on')).toBe(true);
    expect(browser.storage.local.set).toHaveBeenCalledWith({ stampResultsPages: true });
  });
});

describe('sites list', () => {
  it('shows an empty state when no host is remembered', () => {
    expect(byId('sites-list').textContent).toContain("haven't approved");
  });

  it('renders a Forget button per remembered host, which removes it on click', async () => {
    const { getRememberedHosts, forgetHost } = await import('../lib/site-memory');
    vi.mocked(getRememberedHosts).mockResolvedValueOnce(['acme.com']);

    await renderSites();

    // Match the host EXACTLY (via the dedicated `.set-title` node), not a
    // substring of the row's whole text — a row whose host merely CONTAINS
    // "acme.com" (e.g. "notacme.com" or "acme.com.evil.test") must not match.
    const row = Array.from(byId('sites-list').querySelectorAll('.set-row')).find(
      (r) => r.querySelector('.set-title')?.textContent?.trim() === 'acme.com'
    );
    expect(row).toBeDefined();
    row!.querySelector<HTMLButtonElement>('button')!.click();

    expect(forgetHost).toHaveBeenCalledWith('acme.com');
  });
});

describe('what the extension may do', () => {
  it('renders three toggles, all disabled/off while settings.get has not answered', async () => {
    await flush();
    const rows = byId('permissions-list').querySelectorAll('.set-row');
    expect(rows).toHaveLength(3);
    expect(byId('permissions-list').textContent).toContain('Unknown until connected');
    const toggles = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle');
    expect(toggles).toHaveLength(3);
    for (const toggle of toggles) {
      expect(toggle.disabled).toBe(true);
      expect(toggle.getAttribute('aria-checked')).toBe('false');
    }
  });

  it('fetches settings.get and renders the live values', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: true, aiAssist: false, autotrack: false } },
    });

    // Drive it via the onConnected dep (module-load's own settings.get was
    // already consumed by the default `not configured` mock).
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    if (!onConnected) throw new Error('onConnected dep not passed to mountConnectionStatus');
    onConnected();
    await flush();

    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({ kind: 'settingsGet' });
    const toggles = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle');
    expect(toggles[0]!.classList.contains('on')).toBe(true);
    expect(toggles[0]!.disabled).toBe(false);
    expect(toggles[1]!.classList.contains('on')).toBe(false);
  });

  it('clicking a toggle flips it optimistically and sends settings.set', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: false, aiAssist: false, autotrack: false } },
    });
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    onConnected!();
    await flush();

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsSet',
      result: { ok: true, settings: { autofill: true, aiAssist: false, autotrack: false } },
    });

    const toggle = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    expect(toggle.classList.contains('on')).toBe(false);
    toggle.click();
    // Optimistic flip happens synchronously, before the request settles.
    expect(toggle.classList.contains('on')).toBe(true);
    await flush();

    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({
      kind: 'settingsSet',
      key: 'autofill',
      enabled: true,
    });
    expect(toggle.classList.contains('on')).toBe(true);
  });

  it('rolls back the optimistic flip on a desktop-side refusal', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: false, aiAssist: false, autotrack: false } },
    });
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    onConnected!();
    await flush();

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsSet',
      result: { ok: false, error: 'invalid_settings_request' },
    });

    const toggle = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    toggle.click();
    expect(toggle.classList.contains('on')).toBe(true); // optimistic
    await flush();

    // `renderPermissions` rebuilds the row set on rollback, so re-query
    // rather than reuse the now-detached `toggle` reference.
    const afterRollback =
      byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    expect(afterRollback.classList.contains('on')).toBe(false);
  });

  it('ignores a rapid second click on the same toggle while its request is in flight, sending settingsSet exactly once, then re-enables the toggles after the reply', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: false, aiAssist: false, autotrack: false } },
    });
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    onConnected!();
    await flush();

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsSet',
      result: { ok: true, settings: { autofill: true, aiAssist: false, autotrack: false } },
    });
    vi.mocked(browser.runtime.sendMessage).mockClear();

    const toggle = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    toggle.click();
    toggle.click(); // rapid double-click before the first reply settles

    expect(browser.runtime.sendMessage).toHaveBeenCalledTimes(1);
    for (const t of byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')) {
      expect(t.disabled).toBe(true);
    }

    await flush();

    expect(browser.runtime.sendMessage).toHaveBeenCalledTimes(1);
    for (const t of byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')) {
      expect(t.disabled).toBe(false);
    }
  });

  it('re-enables all toggles after an error reply rolls the optimistic flip back', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: false, aiAssist: false, autotrack: false } },
    });
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    onConnected!();
    await flush();

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsSet',
      result: { ok: false, error: 'invalid_settings_request' },
    });

    const toggle = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    toggle.click();

    for (const t of byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')) {
      expect(t.disabled).toBe(true);
    }

    await flush();

    for (const t of byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')) {
      expect(t.disabled).toBe(false);
    }
  });

  it('re-runs settings.get and re-renders on a disconnected→connected transition (onConnected)', async () => {
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    if (!onConnected) throw new Error('onConnected dep not passed to mountConnectionStatus');

    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: true, aiAssist: false, autotrack: false } },
    });

    onConnected();
    await flush();

    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({ kind: 'settingsGet' });
    const toggles = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle');
    expect(toggles[0]!.classList.contains('on')).toBe(true);
  });

  it('shows a user-facing error on a refused settings.set, hiding it again on the next success', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: false, aiAssist: false, autotrack: false } },
    });
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    onConnected!();
    await flush();

    expect(byId('permissions-error').hidden).toBe(true);

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsSet',
      result: { ok: false, error: 'invalid_settings_request' },
    });

    const toggle = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    toggle.click();
    await flush();

    expect(byId('permissions-error').hidden).toBe(false);
    expect(byId('permissions-error').textContent).toMatch(/couldn't change/i);

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsSet',
      result: { ok: true, settings: { autofill: true, aiAssist: false, autotrack: false } },
    });
    // `renderPermissions` rebuilds the row set on rollback, so re-query.
    const afterRollback =
      byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle')[0]!;
    afterRollback.click();
    await flush();

    expect(byId('permissions-error').hidden).toBe(true);
  });

  it('shows a user-facing error when settings.get fails, hiding it again on the next success', async () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: false,
      error: 'not configured',
    });
    const { mountConnectionStatus } = await import('../connection-status/connection-status');
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    onConnected!();
    await flush();

    expect(byId('permissions-error').hidden).toBe(false);
    expect(byId('permissions-error').textContent).toMatch(/couldn't read/i);

    vi.mocked(browser.runtime.sendMessage).mockResolvedValueOnce({
      ok: true,
      kind: 'settingsGet',
      result: { ok: true, settings: { autofill: true, aiAssist: false, autotrack: false } },
    });
    onConnected!();
    await flush();

    expect(byId('permissions-error').hidden).toBe(true);
  });
});

describe('shortcuts (no manifest commands declared)', () => {
  it('shows the fallback line + a button to the browser shortcut settings', () => {
    expect(byId('shortcuts-list').textContent).toContain('No shortcuts are declared yet.');
    const btn = byId('shortcuts-list').querySelector<HTMLButtonElement>('button')!;
    btn.click();
    expect(browser.tabs.create).toHaveBeenCalledWith({ url: 'chrome://extensions/shortcuts' });
  });
});

describe('unpair', () => {
  beforeEach(() => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
  });

  it('sends clearToken on click', () => {
    byId<HTMLButtonElement>('btn-unpair').click();
    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({ kind: 'clearToken' });
  });
});
