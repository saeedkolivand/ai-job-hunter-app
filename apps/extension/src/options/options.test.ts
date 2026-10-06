/**
 * Unit tests for the Settings page controller: render, the theme segmented
 * control persisting + applying `data-theme`, the sites list (empty state +
 * Forget), and the LIVE "What the extension may do" toggles (PR1/PR4, R7 —
 * four switches; see options.ts's own doc).
 */

import { describe, expect, it, vi } from 'vitest';
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

const toggleRow = (id: string, title: string): string => `
  <div class="set-row toggle-row">
    <p id="title-${id}">${title}</p>
    <button id="toggle-${id}" role="switch" aria-checked="false" aria-labelledby="title-${id}"></button>
  </div>`;

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
    ${toggleRow('fit-badge', 'Show the on-page fit badge after Check fit')}
    ${toggleRow('stamp-results', 'Stamp saved/applied on results pages')}
    <div id="shortcuts-list"></div>
    <p id="about-version"></p>
    <a id="link-privacy" href="#"></a>
    <a id="link-help" href="#"></a>
    <a id="link-source" href="#"></a>
    <a id="link-report" href="#"></a>
  `;

// Vitest 5 clears every mock before each test by default, which would wipe
// the module-load-time `mountConnectionStatus(...)` call this file asserts
// on — opt out, same rationale as sidepanel/sidepanel/panel.test.ts's identical guard.
vi.setConfig({ clearMocks: false });

const { renderSites } = await import('./options');

const { mountConnectionStatus } = await import('../connection-status/connection-status');

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));
const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const $ = (selector: string) => document.querySelector<HTMLButtonElement>(selector)!;

describe('render', () => {
  it('mounts connection-status against the pill + views hosts', () => {
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
    $('[data-theme-choice="dark"]').click();
    await flush();

    expect(document.documentElement.dataset.theme).toBe('dark');
    expect($('[data-theme-choice="dark"]').classList.contains('active')).toBe(true);
    expect($('[data-theme-choice="system"]').classList.contains('active')).toBe(false);
  });

  it('persists the choice to storage.local', () => {
    $('[data-theme-choice="light"]').click();
    expect(browser.storage.local.set).toHaveBeenCalledWith({ theme: 'light' });
  });
});

describe('default panel tab segmented control', () => {
  it('persists the choice and marks it active', () => {
    $('[data-tab-choice="answers"]').click();

    expect(browser.storage.local.set).toHaveBeenCalledWith({ defaultPanelTab: 'answers' });
    expect($('[data-tab-choice="answers"]').classList.contains('active')).toBe(true);
  });
});

describe('appearance toggles', () => {
  it.each([
    ['fit-badge', 'showFitBadge'],
    ['stamp-results', 'stampResultsPages'],
  ])('toggles the %s switch on click and persists it', (name, storageKey) => {
    const btn = byId<HTMLButtonElement>('toggle-' + name);
    expect(btn.classList.contains('on')).toBe(false);

    btn.click();

    expect(btn.classList.contains('on')).toBe(true);
    expect(btn.getAttribute('aria-checked')).toBe('true');
    expect(browser.storage.local.set).toHaveBeenCalledWith({ [storageKey]: true });
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
  const send = vi.mocked(browser.runtime.sendMessage);
  const toggles = (): HTMLButtonElement[] => [
    ...byId('permissions-list').querySelectorAll<HTMLButtonElement>('.toggle'),
  ];
  const settings = (over: Record<string, boolean> = {}) => ({
    autofill: false,
    aiAssist: false,
    autotrack: false,
    ...over,
  });
  const got = (over?: Record<string, boolean>) => ({
    ok: true,
    kind: 'settingsGet',
    result: { ok: true, settings: settings(over) },
  });
  const set = (over?: Record<string, boolean>) => ({
    ok: true,
    kind: 'settingsSet',
    result: { ok: true, settings: settings(over) },
  });
  const REFUSED = {
    ok: true,
    kind: 'settingsSet',
    result: { ok: false, error: 'invalid_settings_request' },
  };

  /** Fire the `onConnected` dep (a disconnected→connected transition) with
   *  `reply` as the answer to the `settings.get` it triggers. */
  async function connect(reply: unknown): Promise<void> {
    send.mockClear();
    send.mockResolvedValueOnce(reply as never);
    const onConnected = vi.mocked(mountConnectionStatus).mock.calls[0]?.[2]?.onConnected;
    if (!onConnected) throw new Error('onConnected dep not passed to mountConnectionStatus');
    onConnected();
    await flush();
  }

  it('renders four toggles, all disabled/off while settings.get has not answered', async () => {
    await flush();
    const rows = byId('permissions-list').querySelectorAll('.set-row');
    expect(rows).toHaveLength(4);
    expect(byId('permissions-list').textContent).toContain('Unknown until connected');
    expect(toggles()).toHaveLength(4);
    for (const toggle of toggles()) {
      expect(toggle.disabled).toBe(true);
      expect(toggle.getAttribute('aria-checked')).toBe('false');
    }
  });

  it('the fourth toggle (saveAnswersOnSubmit, PR4) round-trips through settings.get/settings.set, while auto-track is on', async () => {
    await connect(got({ autotrack: true, saveAnswersOnSubmit: false }));
    send.mockResolvedValueOnce(set({ autotrack: true, saveAnswersOnSubmit: true }) as never);

    const toggle = toggles()[3]!;
    expect(toggle.classList.contains('on')).toBe(false);
    expect(toggle.disabled).toBe(false);
    toggle.click();
    await flush();

    expect(send).toHaveBeenCalledWith({
      kind: 'settingsSet',
      key: 'saveAnswersOnSubmit',
      enabled: true,
    });
    expect(toggle.classList.contains('on')).toBe(true);
  });

  it('disables saveAnswersOnSubmit and explains why when auto-track is off (it structurally cannot capture anything without it — the submit-watcher is armed off the same opt-in)', async () => {
    await connect(got({ saveAnswersOnSubmit: true }));

    const toggle = toggles()[3]!;
    expect(toggle.disabled).toBe(true);
    // Reflects the real (stuck-on) value rather than lying about it.
    expect(toggle.classList.contains('on')).toBe(true);
    const row = toggle.closest('.set-row')!;
    expect(row.textContent).toMatch(/turn on auto-track/i);

    send.mockClear();
    toggle.click();
    await flush();
    expect(send).not.toHaveBeenCalled();
  });

  it('fetches settings.get and renders the live values', async () => {
    // Driven via the onConnected dep (module-load's own settings.get was
    // already consumed by the default `not configured` mock).
    await connect(got({ autofill: true }));

    expect(send).toHaveBeenCalledWith({ kind: 'settingsGet' });
    expect(toggles()[0]!.classList.contains('on')).toBe(true);
    expect(toggles()[0]!.disabled).toBe(false);
    expect(toggles()[1]!.classList.contains('on')).toBe(false);
  });

  it('clicking a toggle flips it optimistically and sends settings.set', async () => {
    await connect(got());
    send.mockResolvedValueOnce(set({ autofill: true }) as never);

    const toggle = toggles()[0]!;
    expect(toggle.classList.contains('on')).toBe(false);
    toggle.click();
    // Optimistic flip happens synchronously, before the request settles.
    expect(toggle.classList.contains('on')).toBe(true);
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'settingsSet', key: 'autofill', enabled: true });
    expect(toggle.classList.contains('on')).toBe(true);
  });

  it('rolls back the optimistic flip on a desktop-side refusal', async () => {
    await connect(got());
    send.mockResolvedValueOnce(REFUSED as never);

    toggles()[0]!.click();
    expect(toggles()[0]!.classList.contains('on')).toBe(true); // optimistic
    await flush();

    // `renderPermissions` rebuilds the row set on rollback, so the toggle is
    // re-queried rather than reusing a now-detached reference.
    expect(toggles()[0]!.classList.contains('on')).toBe(false);
  });

  it('ignores a rapid second click on the same toggle while its request is in flight, sending settingsSet exactly once, then re-enables the toggles after the reply', async () => {
    await connect(got());
    send.mockResolvedValueOnce(set({ autofill: true }) as never);
    send.mockClear();

    const toggle = toggles()[0]!;
    toggle.click();
    toggle.click(); // rapid double-click before the first reply settles

    expect(send).toHaveBeenCalledTimes(1);
    for (const t of toggles()) expect(t.disabled).toBe(true);

    await flush();

    // Index 3 (saveAnswersOnSubmit) stays disabled on purpose — this fixture's
    // autotrack is off, and that toggle is gated on it (see options.ts).
    expect(send).toHaveBeenCalledTimes(1);
    toggles().forEach((t, i) => expect(t.disabled).toBe(i === 3));
  });

  it('re-enables all toggles after an error reply rolls the optimistic flip back', async () => {
    await connect(got());
    send.mockResolvedValueOnce(REFUSED as never);

    toggles()[0]!.click();
    for (const t of toggles()) expect(t.disabled).toBe(true);

    await flush();

    // Index 3 (saveAnswersOnSubmit) stays disabled on purpose — this fixture's
    // autotrack is off, and that toggle is gated on it (see options.ts).
    toggles().forEach((t, i) => expect(t.disabled).toBe(i === 3));
  });

  it('re-runs settings.get and re-renders on a disconnected→connected transition (onConnected)', async () => {
    await connect(got({ autofill: true }));

    expect(send).toHaveBeenCalledWith({ kind: 'settingsGet' });
    expect(toggles()[0]!.classList.contains('on')).toBe(true);
  });

  it('shows a user-facing error on a refused settings.set, hiding it again on the next success', async () => {
    await connect(got());
    expect(byId('permissions-error').hidden).toBe(true);

    send.mockResolvedValueOnce(REFUSED as never);
    toggles()[0]!.click();
    await flush();

    expect(byId('permissions-error').hidden).toBe(false);
    expect(byId('permissions-error').textContent).toMatch(/couldn't change/i);

    send.mockResolvedValueOnce(set({ autofill: true }) as never);
    // `renderPermissions` rebuilds the row set on rollback, so re-query.
    toggles()[0]!.click();
    await flush();

    expect(byId('permissions-error').hidden).toBe(true);
  });

  it('shows a user-facing error when settings.get fails, hiding it again on the next success', async () => {
    await connect({ ok: false, error: 'not configured' });

    expect(byId('permissions-error').hidden).toBe(false);
    expect(byId('permissions-error').textContent).toMatch(/couldn't read/i);

    await connect(got({ autofill: true }));

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
  it('sends clearToken on click', () => {
    vi.mocked(browser.runtime.sendMessage).mockClear();
    byId<HTMLButtonElement>('btn-unpair').click();
    expect(browser.runtime.sendMessage).toHaveBeenCalledWith({ kind: 'clearToken' });
  });
});
