/**
 * AutopilotPage — Apply navigation
 *
 * The apply flow is now a real route (`/autopilot/apply`), not an in-body view
 * swap. Clicking "Apply" on a found job must (1) write the apply target into the
 * session store and (2) navigate to `/autopilot/apply` — it must NOT render
 * ApplyPage inline.
 *
 * Strategy:
 *  - `AutopilotCard` is stubbed to expose an "apply" button that calls its
 *    `onApply(job)` prop, so we can trigger the handler without the real card.
 *  - `useAutopilots` returns one autopilot so a card renders.
 *  - `useNavigate` and the real session store are observed.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import type { Autopilot, AutopilotFoundJob } from '@ajh/shared';

import { useSessionStore } from '@/store/session-store';

import { AutopilotPage } from './index';

// ── i18n ──────────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

// ── Router ────────────────────────────────────────────────────────────────────

const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('@/routes/autopilot.index', () => ({
  Route: { useSearch: () => ({}) },
}));

// ── Services ──────────────────────────────────────────────────────────────────

const JOB = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/42',
  description: 'Build cool things.',
  score: 85,
  foundAt: Date.now(),
} as unknown as AutopilotFoundJob;

const AUTOPILOT = {
  _id: 'ap-1',
  resumeText: 'base resume',
  target: { board: 'linkedin' },
} as unknown as Autopilot;

vi.mock('@/services', async (importOriginal) => {
  const orig = await importOriginal();
  return {
    ...(orig as object),
    useAutopilots: () => ({ data: [AUTOPILOT], isLoading: false }),
    useInvalidateAutopilots: () => vi.fn(),
  };
});

// ── Heavy sub-component stubs ─────────────────────────────────────────────────
// AutopilotCard is stubbed to expose an "apply" trigger wired to its onApply prop.

vi.mock('@/features/autopilot/components/AutopilotCard', () => ({
  AutopilotCard: ({ onApply }: { onApply: (job: AutopilotFoundJob) => void }) => (
    <div data-testid="card-apply" onClick={() => onApply(JOB)}>
      apply
    </div>
  ),
}));

vi.mock('@/features/autopilot/components/CreationWizard', () => ({
  CreationWizard: () => <div data-testid="creation-wizard" />,
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
    autopilot: {
      ...s.autopilot,
      apply: null,
      applyWizardStep: 5,
      applyWizardForm: { resume: 'stale' } as never,
      creating: false,
      focusedId: null,
    },
  }));
  mockNavigate.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('AutopilotPage — Apply navigation', () => {
  it('clicking Apply sets the apply target and resets the wizard slice in the store', async () => {
    await act(async () => {
      render(<AutopilotPage />);
    });

    await act(async () => {
      screen.getByTestId('card-apply').click();
    });

    const { autopilot } = useSessionStore.getState();
    expect(autopilot.apply).toEqual({ job: JOB, resumeText: 'base resume', board: 'linkedin' });
    expect(autopilot.applyWizardStep).toBe(0);
    expect(autopilot.applyWizardForm).toBeNull();
  });

  it('clicking Apply navigates to /autopilot/apply (no inline ApplyPage)', async () => {
    await act(async () => {
      render(<AutopilotPage />);
    });

    await act(async () => {
      screen.getByTestId('card-apply').click();
    });

    expect(mockNavigate).toHaveBeenCalledWith({ to: '/autopilot/apply' });
    // The list body stays — ApplyPage is no longer rendered inline by this page.
    expect(screen.queryByTestId('apply-page')).not.toBeInTheDocument();
    expect(screen.getByTestId('card-apply')).toBeInTheDocument();
  });
});
