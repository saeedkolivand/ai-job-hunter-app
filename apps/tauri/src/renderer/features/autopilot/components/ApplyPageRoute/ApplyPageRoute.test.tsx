/**
 * ApplyPageRoute — route-level apply flow
 *
 * Strategy:
 *  - ApplyPage is stubbed (its own behavior is covered by ApplyPage.test.tsx);
 *    here we only assert the route's two jobs: render ApplyPage when an apply
 *    target is set, and bounce to /autopilot when it is not.
 *  - `useNavigate` is a controlled mock.
 *  - The real Zustand session store drives `autopilot.apply` so we exercise the
 *    actual selector wiring.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';

import type { AutopilotFoundJob } from '@ajh/shared';

import type { TailorWizardState } from '@/features/documents/components/TailorFlow/lib/tailor-state';
import { useSessionStore } from '@/store/session-store';

import { ApplyPageRoute } from './index';

// ── Router ────────────────────────────────────────────────────────────────────

const mockNavigate = vi.fn();

vi.mock('@tanstack/react-router', () => ({
  useNavigate: () => mockNavigate,
}));

// ── ApplyPage stub — surfaces forwarded props as data attributes ──────────────

vi.mock('../ApplyPage', () => ({
  ApplyPage: ({
    job,
    resumeText,
    board,
    onBack,
  }: {
    job: AutopilotFoundJob;
    resumeText?: string;
    board: string;
    onBack: () => void;
  }) => (
    <div
      data-testid="apply-page"
      data-board={board}
      data-job-title={job.title}
      data-job-url={job.url}
      data-resume-text={resumeText}
      onClick={onBack}
    >
      apply
    </div>
  ),
}));

// ── Fixtures ──────────────────────────────────────────────────────────────────

const JOB: AutopilotFoundJob = {
  title: 'Senior Engineer',
  company: 'Acme',
  url: 'https://acme.com/jobs/42',
  description: 'Build cool things.',
  score: 85,
  foundAt: Date.now(),
};

/** Stale wizard form with non-default values — used to verify handleBack resets to null. */
const STALE_WIZARD_FORM: TailorWizardState = {
  resume: 'stale resume text',
  outputType: 'resume',
  researchCompany: true,
};

beforeEach(() => {
  useSessionStore.setState((s) => ({
    autopilot: { ...s.autopilot, apply: null, applyWizardStep: 0, applyWizardForm: null },
  }));
  mockNavigate.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('ApplyPageRoute', () => {
  it('renders ApplyPage with the apply target when one is set', async () => {
    useSessionStore.setState((s) => ({
      autopilot: {
        ...s.autopilot,
        apply: { job: JOB, resumeText: 'base resume', board: 'linkedin' },
      },
    }));

    await act(async () => {
      render(<ApplyPageRoute />);
    });

    const page = screen.getByTestId('apply-page');
    expect(page).toBeInTheDocument();
    // FIX 3: assert all three forwarded props reach the stub
    expect(page).toHaveAttribute('data-board', 'linkedin');
    expect(page).toHaveAttribute('data-job-title', 'Senior Engineer');
    expect(page).toHaveAttribute('data-job-url', 'https://acme.com/jobs/42');
    expect(page).toHaveAttribute('data-resume-text', 'base resume');
    // No redirect while a target is present.
    expect(mockNavigate).not.toHaveBeenCalled();
  });

  it('redirects to /autopilot (replace) when there is no apply target', async () => {
    let container!: HTMLElement;

    await act(async () => {
      ({ container } = render(<ApplyPageRoute />));
    });

    // FIX 2: component returns null — the container must be empty
    expect(container).toBeEmptyDOMElement();
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/autopilot', replace: true });
  });

  it('onBack clears the apply target and navigates back to /autopilot', async () => {
    // FIX 1: seed NON-DEFAULT wizard values so post-click assertions are not vacuous
    useSessionStore.setState((s) => ({
      autopilot: {
        ...s.autopilot,
        apply: { job: JOB, resumeText: 'base', board: 'linkedin' },
        applyWizardStep: 3,
        applyWizardForm: STALE_WIZARD_FORM,
      },
    }));

    await act(async () => {
      render(<ApplyPageRoute />);
    });

    await act(async () => {
      screen.getByTestId('apply-page').click();
    });

    const { autopilot } = useSessionStore.getState();
    // All three fields must be reset — each assertion catches a distinct regression
    expect(autopilot.apply).toBeNull();
    expect(autopilot.applyWizardStep).toBe(0);
    expect(autopilot.applyWizardForm).toBeNull();
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/autopilot' });
  });
});
