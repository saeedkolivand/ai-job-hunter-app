/**
 * Unit tests for the Settings page controller (PR0 §5): render, the theme
 * segmented control persisting + applying `data-theme`, the sites list
 * (empty state + Forget), and the read-only "What the extension may do"
 * rows (three, not the mockup's four — see options.ts's own doc for why).
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
    <div id="theme-seg">
      <button type="button" data-theme-choice="system">System</button>
      <button type="button" data-theme-choice="light">Light</button>
      <button type="button" data-theme-choice="dark">Dark</button>
    </div>
    <div id="default-tab-seg">
      <button type="button" data-tab-choice="job">Job</button>
      <button type="button" data-tab-choice="answers">Answers</button>
    </div>
    <button id="toggle-fit-badge" role="switch" aria-checked="false"></button>
    <button id="toggle-stamp-results" role="switch" aria-checked="false"></button>
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
});

describe('sites list', () => {
  it('shows an empty state when no host is remembered', () => {
    expect(byId('sites-list').textContent).toContain("haven't approved");
  });

  it('renders a Forget button per remembered host, which removes it on click', async () => {
    const { getRememberedHosts, forgetHost } = await import('../lib/site-memory');
    vi.mocked(getRememberedHosts).mockResolvedValueOnce(['acme.com']);

    await renderSites();

    const row = Array.from(byId('sites-list').querySelectorAll('.set-row')).find((r) =>
      r.textContent?.includes('acme.com')
    );
    expect(row).toBeDefined();
    row!.querySelector<HTMLButtonElement>('button')!.click();

    expect(forgetHost).toHaveBeenCalledWith('acme.com');
  });
});

describe('what the extension may do', () => {
  it('renders three rows, all "Unknown until connected" while autofillCheck has not answered', async () => {
    await flush();
    const rows = byId('permissions-list').querySelectorAll('.set-row');
    expect(rows).toHaveLength(3);
    expect(byId('permissions-list').textContent).toContain('Unknown until connected');
  });

  it('each row has a "Change in app →" caption that opens the deep link', () => {
    const captions = byId('permissions-list').querySelectorAll<HTMLButtonElement>('.link');
    expect(captions.length).toBeGreaterThan(0);
    captions[0]!.click();
    expect(browser.tabs.create).toHaveBeenCalledWith({ url: 'ajh://settings/extension' });
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
