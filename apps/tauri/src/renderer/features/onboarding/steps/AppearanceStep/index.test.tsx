/**
 * AppearanceStep — render + interaction tests.
 *
 * Strategy:
 *  - @ajh/translations → key-passthrough t() (mirrors AppearanceCard.test.tsx).
 *  - @ajh/ui → real module spread; applyThemeAnimated replaced with a spy so
 *    no localStorage / document.documentElement writes happen.
 *    getThemePrefs is also spied on to return a deterministic initial state.
 *  - useSystemAccent is mocked via @/services:
 *      * supported:false → System accent chip hidden (deterministic assertions)
 *      * supported:true  → System chip visible (second describe block)
 *  - SCHEMES has 3 entries (light/dark/system); ACCENTS has 8 entries.
 *  - withProviders + createMockClient supply the React Query / AppClient context.
 *
 * noUncheckedIndexedAccess: every array access is guarded.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { createMockClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stubs — default: system accent not supported ──────────────────────

let systemAccentSupported = false;
let systemAccentColor: string | null = null;

vi.mock('@/services', () => ({
  useSystemAccent: () => ({
    data: { supported: systemAccentSupported, color: systemAccentColor },
  }),
}));

// ── @ajh/ui: spread real module; spy on applyThemeAnimated + getThemePrefs ────

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...(actual as object),
    applyThemeAnimated: vi.fn(),
    // Return a deterministic initial prefs so useState(() => getThemePrefs())
    // produces consistent starting state across all tests.
    getThemePrefs: vi.fn(() => ({
      scheme: 'system' as const,
      accentSource: 'default' as const,
      accentColor: undefined,
      accentColor2: undefined,
    })),
  };
});

// ── component under test (imported AFTER mocks) ───────────────────────────────

import * as AjhUiModule from '@ajh/ui';

import { AppearanceStep } from './index';

const applyThemeAnimatedSpy = vi.mocked(AjhUiModule.applyThemeAnimated);

// ── helpers ───────────────────────────────────────────────────────────────────

function renderStep(overrides: Partial<Parameters<typeof AppearanceStep>[0]> = {}) {
  const onNext = vi.fn();
  const onBack = vi.fn();
  const client = createMockClient();

  const props = {
    onNext,
    onBack,
    direction: 1 as const,
    stepIndex: 5,
    totalSteps: 6,
    ...overrides,
  };

  const result = render(<AppearanceStep {...props} />, { wrapper: withProviders(client) });
  return { ...result, onNext, onBack };
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('AppearanceStep — scheme radiogroup', () => {
  it('renders a radiogroup for colour scheme', () => {
    renderStep();
    expect(
      screen.getByRole('radiogroup', { name: 'settings.appearance.scheme' })
    ).toBeInTheDocument();
  });

  it('renders exactly 3 scheme radio buttons (light, dark, system)', () => {
    renderStep();
    const radiogroup = screen.getByRole('radiogroup', { name: 'settings.appearance.scheme' });
    const radios = radiogroup.querySelectorAll('[role="radio"]');
    expect(radios).toHaveLength(3);
  });

  it('initial scheme matches getThemePrefs (system is aria-checked)', () => {
    renderStep();
    const systemBtn = screen.getByRole('radio', {
      name: 'settings.appearance.system',
    });
    expect(systemBtn.getAttribute('aria-checked')).toBe('true');
  });

  it('clicking the dark scheme radio calls applyThemeAnimated with scheme:dark', async () => {
    applyThemeAnimatedSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    await user.click(screen.getByRole('radio', { name: 'settings.appearance.dark' }));

    expect(applyThemeAnimatedSpy).toHaveBeenCalledTimes(1);
    const call = applyThemeAnimatedSpy.mock.calls.at(0);
    if (!call) throw new Error('expected applyThemeAnimated call');
    const prefs = call[0];
    expect(prefs.scheme).toBe('dark');
  });

  it('clicking the light scheme radio calls applyThemeAnimated with scheme:light', async () => {
    applyThemeAnimatedSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    await user.click(screen.getByRole('radio', { name: 'settings.appearance.light' }));

    const call = applyThemeAnimatedSpy.mock.calls.at(0);
    if (!call) throw new Error('expected applyThemeAnimated call');
    const prefs = call[0];
    expect(prefs.scheme).toBe('light');
  });

  it('aria-checked updates to reflect the newly selected scheme after click', async () => {
    const user = userEvent.setup();
    renderStep();

    const darkBtn = screen.getByRole('radio', { name: 'settings.appearance.dark' });
    expect(darkBtn.getAttribute('aria-checked')).toBe('false');

    await user.click(darkBtn);

    expect(darkBtn.getAttribute('aria-checked')).toBe('true');

    const systemBtn = screen.getByRole('radio', { name: 'settings.appearance.system' });
    expect(systemBtn.getAttribute('aria-checked')).toBe('false');
  });
});

describe('AppearanceStep — accent radiogroup', () => {
  it('renders a radiogroup for accent colour', () => {
    renderStep();
    expect(
      screen.getByRole('radiogroup', { name: 'settings.appearance.accent' })
    ).toBeInTheDocument();
  });

  it('the Default accent chip is aria-checked when initial accentSource is default', () => {
    renderStep();
    const defaultChip = screen.getByRole('radio', {
      name: 'settings.appearance.accentDefault',
    });
    expect(defaultChip.getAttribute('aria-checked')).toBe('true');
  });

  it('clicking a preset swatch calls applyThemeAnimated with accentSource:custom and both hex stops', async () => {
    applyThemeAnimatedSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    // First swatch in ACCENTS is violet: #a855f7 / #6366f1
    const violetBtn = screen.getByRole('radio', {
      name: 'settings.appearance.accentViolet',
    });
    await user.click(violetBtn);

    const call = applyThemeAnimatedSpy.mock.calls.at(0);
    if (!call) throw new Error('expected applyThemeAnimated call');
    const prefs = call[0];
    expect(prefs.accentSource).toBe('custom');
    // Pin the exact hex stops from ACCENTS[violet] in @/constants/appearance.
    // If these values change in the constant the theme engine regresses silently
    // everywhere that reads them — this test catches that propagation.
    expect(prefs.accentColor).toBe('#a855f7');
    expect(prefs.accentColor2).toBe('#6366f1');
  });

  it('aria-checked on default chip becomes false after a custom swatch is clicked', async () => {
    const user = userEvent.setup();
    renderStep();

    const defaultChip = screen.getByRole('radio', {
      name: 'settings.appearance.accentDefault',
    });
    expect(defaultChip.getAttribute('aria-checked')).toBe('true');

    await user.click(screen.getByRole('radio', { name: 'settings.appearance.accentBlue' }));

    expect(defaultChip.getAttribute('aria-checked')).toBe('false');
  });
});

describe('AppearanceStep — system accent chip visibility', () => {
  afterEach(() => {
    systemAccentSupported = false;
    systemAccentColor = null;
  });

  it('System chip is hidden when useSystemAccent returns supported:false', () => {
    systemAccentSupported = false;
    const { container } = renderStep();

    // The System ACCENT chip lives inside the accent radiogroup.
    // The scheme radiogroup also has a 'system' radio (Monitor icon), so we
    // must scope the query to the accent group to avoid a false match.
    const accentRadiogroup = container.querySelector(
      '[role="radiogroup"][aria-label="settings.appearance.accent"]'
    );
    if (!accentRadiogroup) throw new Error('expected accent radiogroup');

    const systemChip = accentRadiogroup.querySelector('[aria-label="settings.appearance.system"]');
    expect(systemChip).not.toBeInTheDocument();
  });

  it('System chip is visible when useSystemAccent returns supported:true', () => {
    systemAccentSupported = true;
    systemAccentColor = '#ff0000';
    renderStep();

    // The System ACCENT chip in the accent radiogroup (not the scheme radiogroup)
    // has aria-label 'settings.appearance.system'
    const accentRadiogroup = screen.getByRole('radiogroup', {
      name: 'settings.appearance.accent',
    });
    const systemChip = accentRadiogroup.querySelector('[aria-label="settings.appearance.system"]');
    expect(systemChip).toBeInTheDocument();
  });

  it('clicking System accent chip calls applyThemeAnimated with accentSource:system', async () => {
    systemAccentSupported = true;
    systemAccentColor = '#0078d4';
    applyThemeAnimatedSpy.mockClear();
    const user = userEvent.setup();
    renderStep();

    const accentRadiogroup = screen.getByRole('radiogroup', {
      name: 'settings.appearance.accent',
    });
    const systemChip = accentRadiogroup.querySelector<HTMLElement>(
      '[aria-label="settings.appearance.system"]'
    );
    if (!systemChip) throw new Error('expected system accent chip');

    await user.click(systemChip);

    const call = applyThemeAnimatedSpy.mock.calls.at(0);
    if (!call) throw new Error('expected applyThemeAnimated call');
    const prefs = call[0];
    expect(prefs.accentSource).toBe('system');
  });
});

describe('AppearanceStep — navigation', () => {
  it('clicking the next button calls onNext', async () => {
    const user = userEvent.setup();
    const { onNext } = renderStep();

    await user.click(screen.getByRole('button', { name: 'onboarding.appearance.next' }));

    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it('clicking the back button calls onBack', async () => {
    const user = userEvent.setup();
    const { onBack } = renderStep();

    await user.click(screen.getByRole('button', { name: 'onboarding.appearance.back' }));

    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
