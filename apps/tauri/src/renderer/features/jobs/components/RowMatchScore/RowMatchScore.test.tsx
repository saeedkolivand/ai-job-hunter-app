/**
 * RowMatchScore — presentational render-state tests.
 *
 * Scores are now on-demand: RowMatchScore renders nothing until the user opens
 * the job and scoreJob fires. There is no "pending" loading placeholder.
 *
 * Two render branches:
 *  - hasResume === false → renders nothing
 *  - score present + hasResume → renders the MatchBand + est. label + info trigger
 *  - no score (not yet opened) → renders nothing
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import type { MatchScore } from '@ajh/shared';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── useRowMatchScore stub ─────────────────────────────────────────────────────

let stubbedRow: { score?: MatchScore; hasResume: boolean } = {
  hasResume: false,
};

vi.mock('@/features/jobs/providers', () => ({
  useRowMatchScore: () => stubbedRow,
}));

// ── component under test ──────────────────────────────────────────────────────

import { RowMatchScore } from './index';

// ── constants ─────────────────────────────────────────────────────────────────

const JOB_ID = 'job-abc';
const RESUME_ID = 'resume-xyz';

const BASE_SCORE: MatchScore = {
  resumeId: RESUME_ID,
  jobId: JOB_ID,
  ats: 84,
  semantic: 80,
  combined: 82,
  gaps: [],
  recommendations: [],
};

function renderRow(row: typeof stubbedRow) {
  stubbedRow = row;
  return render(<RowMatchScore jobId={JOB_ID} />);
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('RowMatchScore — no resume state', () => {
  it('renders nothing when hasResume is false', () => {
    const { container } = renderRow({ hasResume: false });
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing even with a score when hasResume is false', () => {
    const { container } = renderRow({ score: BASE_SCORE, hasResume: false });
    expect(container.firstChild).toBeNull();
  });
});

describe('RowMatchScore — score present', () => {
  it('renders the High MatchBand label for a combined score >= 75', () => {
    const { container } = renderRow({
      score: { ...BASE_SCORE, combined: 82 },
      hasResume: true,
    });
    expect(screen.getByText('jobs.matchBand.High')).toBeInTheDocument();
    // No loading placeholder — on-demand model has no pending spinner.
    expect(container.querySelector('[aria-busy="true"]')).not.toBeInTheDocument();
  });

  it('renders the Medium MatchBand label for a combined score in [50, 74]', () => {
    renderRow({ score: { ...BASE_SCORE, combined: 60 }, hasResume: true });
    expect(screen.getByText('jobs.matchBand.Medium')).toBeInTheDocument();
  });

  it('renders the Low MatchBand label for a combined score < 50', () => {
    renderRow({ score: { ...BASE_SCORE, combined: 30 }, hasResume: true });
    expect(screen.getByText('jobs.matchBand.Low')).toBeInTheDocument();
  });

  it('renders the always-visible estimate label adjacent to the band', () => {
    renderRow({ score: { ...BASE_SCORE, combined: 82 }, hasResume: true });
    expect(screen.getByText('jobs.scoreEst')).toBeInTheDocument();
  });

  it('renders a guidance info trigger with the scoreGuidanceLabel aria-label', () => {
    renderRow({ score: { ...BASE_SCORE, combined: 82 }, hasResume: true });
    expect(screen.getByRole('button', { name: 'jobs.scoreGuidanceLabel' })).toBeInTheDocument();
  });

  it('reveals guidance popover content when the trigger wrapper receives focus', () => {
    const { container } = renderRow({
      score: { ...BASE_SCORE, combined: 82 },
      hasResume: true,
    });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();

    fireEvent.focus(container.firstChild as HTMLElement);

    const tooltip = screen.getByRole('tooltip');
    expect(tooltip).toBeInTheDocument();
    expect(tooltip).toHaveTextContent('jobs.scoreGuidance');
  });
});

describe('RowMatchScore — no score (job not yet opened)', () => {
  it('renders nothing when resume present but job has not been scored yet', () => {
    // On-demand model: unopened rows have no score and show no badge — not an error state.
    const { container } = renderRow({ hasResume: true });
    expect(container.firstChild).toBeNull();
  });
});
