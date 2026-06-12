/**
 * AutopilotPage — ?focus deep-link consumption (Priority 4)
 *
 * Strategy:
 *  - Effect-level test: stub every heavy sub-component (AutopilotCard, CreationWizard,
 *    ApplyPage, EmptyState, useAutopilotRun) so the page renders cheaply.
 *  - `useSearch` backed by a mutable variable so we can set it per-test.
 *  - `useAutopilots` + `useInvalidateAutopilots` are controlled mocks.
 *  - Assert: with `?focus=<id>`, `setAutopilot({ focusedId: focus })` is called
 *    (observed via session-store) and navigate({to:'/autopilot',search:{},replace:true})
 *    is called to clear the param.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { useSessionStore } from '@/store/session-store';

import { AutopilotPage } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router ────────────────────────────────────────────────────────────────────

let currentSearch: Record<string, string | undefined> = {};
const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('@/routes/autopilot', () => ({
  Route: { useSearch: () => currentSearch },
}));

// ── Services ──────────────────────────────────────────────────────────────────

const mockInvalidateAutopilots = vi.fn();

vi.mock('@/services', async (importOriginal) => {
  const orig = await importOriginal();
  return {
    ...(orig as object),
    useAutopilots: () => ({ data: [], isLoading: false }),
    useInvalidateAutopilots: () => mockInvalidateAutopilots,
  };
});

// ── Heavy sub-component stubs ─────────────────────────────────────────────────

vi.mock('@/features/autopilot/components/AutopilotCard', () => ({
  AutopilotCard: () => <div data-testid="autopilot-card" />,
}));

vi.mock('@/features/autopilot/components/CreationWizard', () => ({
  CreationWizard: () => <div data-testid="creation-wizard" />,
}));

vi.mock('@/features/autopilot/components/ApplyPage', () => ({
  ApplyPage: () => <div data-testid="apply-page" />,
}));

vi.mock('@/features/autopilot/components/EmptyState', () => ({
  EmptyState: () => <div data-testid="autopilot-empty-state" />,
}));

vi.mock('@/features/autopilot/hooks/useAutopilotRun', () => ({
  useAutopilotRun: () => ({
    runStates: {},
    stepLogs: {},
    error: null,
    setError: vi.fn(),
    handleRun: vi.fn(),
    handleTogglePause: vi.fn(),
    handleDelete: vi.fn(),
  }),
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="page-transition">{children}</div>
  ),
}));

// ── Store reset ───────────────────────────────────────────────────────────────

beforeEach(() => {
  useSessionStore.setState((s) => ({
    autopilot: { ...s.autopilot, focusedId: null, creating: false },
  }));
  mockNavigate.mockReset();
  mockInvalidateAutopilots.mockReset();
  currentSearch = {};
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('AutopilotPage — ?focus consumption', () => {
  it('sets autopilot.focusedId in session store when ?focus is present', async () => {
    currentSearch = { focus: 'ap-42' };

    await act(async () => {
      render(<AutopilotPage />);
    });

    const { autopilot } = useSessionStore.getState();
    expect(autopilot.focusedId).toBe('ap-42');
  });

  it('clears the ?focus URL param via navigate({to:/autopilot,search:{},replace:true})', async () => {
    currentSearch = { focus: 'ap-42' };

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(mockNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ to: '/autopilot', search: {}, replace: true })
    );
  });

  it('calls useInvalidateAutopilots() to refresh the list when ?focus is set', async () => {
    currentSearch = { focus: 'ap-42' };

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(mockInvalidateAutopilots).toHaveBeenCalledTimes(1);
  });

  it('does nothing when ?focus is absent', async () => {
    currentSearch = {};

    await act(async () => {
      render(<AutopilotPage />);
    });

    expect(mockNavigate).not.toHaveBeenCalled();
    expect(mockInvalidateAutopilots).not.toHaveBeenCalled();
    const { autopilot } = useSessionStore.getState();
    expect(autopilot.focusedId).toBeNull();
  });
});
