/**
 * BrowserDetectedState — render + getBrowserLabel unit tests.
 *
 * Strategy:
 *  - @ajh/translations → key-passthrough t() so assertions target i18n keys,
 *    never raw English strings.
 *  - getBrowserLabel is exported as a pure function — tested directly (no DOM).
 *  - Render tests confirm: heading uses `onboarding.browser.ready`, subtitle
 *    uses `onboarding.browser.willUse`, and the browser card shows the derived
 *    label (not a hardcoded brand).
 *
 * noUncheckedIndexedAccess: no raw array indexing.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { createMockClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @ajh/ui — spread actual so Button / motion helpers resolve correctly ──────
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return { ...(actual as object) };
});

// ── component + util under test ───────────────────────────────────────────────
import { BrowserDetectedState, getBrowserLabel } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────
function renderState(browserPath: string) {
  const onNext = vi.fn();
  const onBack = vi.fn();
  const client = createMockClient();
  const result = render(
    <BrowserDetectedState browserPath={browserPath} onBack={onBack} onNext={onNext} />,
    { wrapper: withProviders(client) }
  );
  return { ...result, onNext, onBack };
}

// ── getBrowserLabel — pure unit tests ─────────────────────────────────────────
describe('getBrowserLabel', () => {
  it('returns Chrome for a native Chrome path', () => {
    expect(getBrowserLabel('/usr/bin/google-chrome')).toBe('Chrome');
  });

  it('returns Chrome for a Windows Chrome path', () => {
    expect(getBrowserLabel('C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe')).toBe(
      'Chrome'
    );
  });

  it('returns Chrome for a Flatpak com.google.Chrome command', () => {
    expect(getBrowserLabel('flatpak run com.google.Chrome')).toBe('Chrome');
  });

  it('returns Brave for a native brave-browser path', () => {
    expect(getBrowserLabel('/usr/bin/brave-browser')).toBe('Brave');
  });

  it('returns Brave for a Flatpak com.brave.Browser command', () => {
    expect(getBrowserLabel('flatpak run com.brave.Browser')).toBe('Brave');
  });

  it('returns Chromium for a chromium path', () => {
    expect(getBrowserLabel('/snap/bin/chromium')).toBe('Chromium');
  });

  it('returns Chromium for a Flatpak org.chromium.Chromium command', () => {
    expect(getBrowserLabel('flatpak run org.chromium.Chromium')).toBe('Chromium');
  });

  it('returns Edge for microsoft-edge path', () => {
    expect(getBrowserLabel('/usr/bin/microsoft-edge')).toBe('Edge');
  });

  it('returns Edge for msedge Windows path', () => {
    expect(
      getBrowserLabel('C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe')
    ).toBe('Edge');
  });

  it('returns Vivaldi for a vivaldi path', () => {
    expect(getBrowserLabel('/opt/vivaldi/vivaldi')).toBe('Vivaldi');
  });

  it('returns null for an unrecognised path', () => {
    expect(getBrowserLabel('/usr/bin/firefox')).toBeNull();
  });

  it('returns null for an empty string', () => {
    expect(getBrowserLabel('')).toBeNull();
  });
});

// ── render — i18n keys, NOT raw English ──────────────────────────────────────
describe('BrowserDetectedState — render', () => {
  it('heading renders the onboarding.browser.ready i18n key', () => {
    renderState('/usr/bin/google-chrome-stable');
    expect(screen.getByRole('heading', { name: 'onboarding.browser.ready' })).toBeInTheDocument();
  });

  it('subtitle renders the onboarding.browser.willUse i18n key', () => {
    renderState('/usr/bin/google-chrome-stable');
    expect(screen.getByText('onboarding.browser.willUse')).toBeInTheDocument();
  });

  it('readyLabel renders the onboarding.browser.readyLabel i18n key', () => {
    renderState('/usr/bin/google-chrome-stable');
    expect(screen.getByText('onboarding.browser.readyLabel')).toBeInTheDocument();
  });

  it('shows "Chrome" as the browser name for a Chrome path — not a hardcoded brand', () => {
    renderState('/usr/bin/google-chrome');
    expect(screen.getByText('Chrome')).toBeInTheDocument();
  });

  it('shows "Brave" for a Flatpak Brave path, not "Chrome"', () => {
    renderState('flatpak run com.brave.Browser');
    expect(screen.getByText('Brave')).toBeInTheDocument();
    expect(screen.queryByText('Chrome')).not.toBeInTheDocument();
  });

  it('shows "Chromium" for a Flatpak Chromium path', () => {
    renderState('flatpak run org.chromium.Chromium');
    expect(screen.getByText('Chromium')).toBeInTheDocument();
  });

  it('falls back to the onboarding.browser.detected i18n key for an unknown path', () => {
    renderState('/usr/bin/firefox');
    // With key-passthrough t(), the fallback renders as the key string
    expect(screen.getByText('onboarding.browser.detected')).toBeInTheDocument();
  });

  it('renders the next navigation button', () => {
    renderState('/usr/bin/google-chrome');
    expect(screen.getByRole('button', { name: /onboarding\.browser\.next/ })).toBeInTheDocument();
  });

  it('does not render raw English "Chrome Ready"', () => {
    renderState('/usr/bin/google-chrome');
    expect(screen.queryByText('Chrome Ready')).not.toBeInTheDocument();
  });

  it('does not render raw English "Google Chrome" brand label', () => {
    renderState('/usr/bin/google-chrome');
    expect(screen.queryByText('Google Chrome')).not.toBeInTheDocument();
  });
});
