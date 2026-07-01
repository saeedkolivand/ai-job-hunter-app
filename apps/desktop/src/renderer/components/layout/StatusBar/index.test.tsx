/**
 * StatusBar — structural tests (feat/accent-gradients).
 *
 * Strategy:
 *  - All hooks that reach IPC / QueryClient / Zustand persistence are stubbed
 *    at the module level (same pattern as NotificationBell.test.tsx and
 *    ModelSelector.test.tsx).
 *  - @tanstack/react-router useRouter is mocked with a controllable navigate spy.
 *  - @/store/session-store useSessionStore is mocked; setSettings is a vi.fn()
 *    spy so we can assert it receives { activeSection: 'ai' }.
 *  - @/providers/CapabilityProvider useCapabilities returns a canned value.
 *  - @/services useWorkerActivity returns idle state.
 *  - @/store/preferences-store useAIModel / useAiProviderConfig return minimal stubs.
 *  - @/hooks/use-kind-label-map useKindLabelMap returns an empty map.
 *  - motion/react is globally shimmed — AnimatePresence renders children synchronously.
 *  - HoverPopover internally calls createPortal; portalled content is reachable
 *    via screen queries that search the full document.
 *
 * Covers (feat/accent-gradients):
 *  - Clicking the AI-settings button calls setSettings({ activeSection: 'ai' }).
 *  - Clicking the AI-settings button navigates to ROUTES.SETTINGS.
 *  - The AI-settings button is identifiable via aria-label="AI settings".
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}));

// ── Router ────────────────────────────────────────────────────────────────────

const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

// ── session-store — spy on setSettings ────────────────────────────────────────

const mockSetSettings = vi.fn();

vi.mock('@/store/session-store', () => ({
  useSessionStore: (selector: (s: { setSettings: typeof mockSetSettings }) => unknown) =>
    selector({ setSettings: mockSetSettings }),
}));

// ── CapabilityProvider ────────────────────────────────────────────────────────

vi.mock('@/providers/CapabilityProvider', () => ({
  useCapabilities: () => ({
    ai: { ready: true, model: 'llama3.2' },
    data: { ready: true, sqlite: true, vector: true },
    initialized: true,
  }),
}));

// ── Worker activity ───────────────────────────────────────────────────────────

vi.mock('@/services', async (importOriginal) => {
  const orig = await importOriginal<Record<string, unknown>>();
  return {
    ...(orig as object),
    useWorkerActivity: () => ({
      isActive: false,
      active: 0,
      queued: 0,
      running: [],
      queuedJobs: [],
    }),
  };
});

// ── Preferences store ─────────────────────────────────────────────────────────

vi.mock('@/store/preferences-store', () => ({
  useAIModel: () => ({ defaultModel: 'llama3.2' }),
  useAiProviderConfig: () => ({
    activeProvider: 'ollama',
    providers: { ollama: { model: '' } },
  }),
}));

// ── Kind label map ────────────────────────────────────────────────────────────

vi.mock('@/hooks/use-kind-label-map', () => ({
  useKindLabelMap: () => ({}),
}));

// ── component under test ──────────────────────────────────────────────────────

import { ROUTES } from '@/constants/routes';

import { StatusBar } from './index';

// ── helpers ───────────────────────────────────────────────────────────────────

function renderStatusBar() {
  return render(<StatusBar />);
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  mockNavigate.mockReset();
  mockSetSettings.mockReset();
});

// ── AI-settings button (feat/accent-gradients) ────────────────────────────────

describe('StatusBar — AI-settings button', () => {
  it('renders a button with aria-label="AI settings"', () => {
    renderStatusBar();
    expect(screen.getByRole('button', { name: 'AI settings' })).toBeInTheDocument();
  });

  it('clicking "AI settings" calls setSettings with { activeSection: "ai" }', () => {
    renderStatusBar();
    fireEvent.click(screen.getByRole('button', { name: 'AI settings' }));
    expect(mockSetSettings).toHaveBeenCalledOnce();
    expect(mockSetSettings).toHaveBeenCalledWith({ activeSection: 'ai' });
  });

  it('clicking "AI settings" navigates to ROUTES.SETTINGS', () => {
    renderStatusBar();
    fireEvent.click(screen.getByRole('button', { name: 'AI settings' }));
    expect(mockNavigate).toHaveBeenCalledOnce();
    expect(mockNavigate).toHaveBeenCalledWith(expect.objectContaining({ to: ROUTES.SETTINGS }));
  });
});

// ── static content smoke checks ───────────────────────────────────────────────

describe('StatusBar — static content', () => {
  it('renders the db-status text "SQLite · LanceDB" when both are ready', () => {
    renderStatusBar();
    expect(screen.getByText('SQLite · LanceDB')).toBeInTheDocument();
  });

  it('renders the offline-capable tagline', () => {
    renderStatusBar();
    expect(screen.getByText('local-first · offline-capable')).toBeInTheDocument();
  });
});
