/**
 * RowMatchScore — presentational render-state tests.
 *
 * RowMatchScore is now presentational: the combined score, pending flag, and
 * hasResume flag are supplied by MatchScoresProvider via useRowMatchScore (one
 * batch call for all filtered postings). These tests stub useRowMatchScore and
 * assert the three render branches:
 *  - hasResume === false → renders nothing
 *  - score present → renders the MatchBand tier label (High/Medium/Low)
 *  - pending (no score yet) → renders the aria-busy loading placeholder
 *  - neither score nor pending → renders nothing
 */
import { describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/dom';
import { render } from '@testing-library/react';

import type { MatchScore } from '@ajh/shared';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── useRowMatchScore stub ─────────────────────────────────────────────────────
// Module-level ref so each test sets it BEFORE render. Never set after render.

let stubbedRow: { score?: MatchScore; pending: boolean; hasResume: boolean } = {
  pending: false,
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

// ── helper — always set stubbedRow BEFORE calling render ──────────────────────

function renderRow(row: typeof stubbedRow) {
  stubbedRow = row;
  return render(<RowMatchScore jobId={JOB_ID} />);
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('RowMatchScore — no resume state', () => {
  it('renders nothing when hasResume is false', () => {
    const { container } = renderRow({ pending: false, hasResume: false });
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing even while pending when hasResume is false', () => {
    const { container } = renderRow({ pending: true, hasResume: false });
    expect(container.firstChild).toBeNull();
  });
});

describe('RowMatchScore — score state', () => {
  it('renders the High MatchBand label for a combined score >= 75', () => {
    const { container } = renderRow({
      score: { ...BASE_SCORE, combined: 82 },
      pending: false,
      hasResume: true,
    });
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeInTheDocument();
  });

  it('renders the Medium MatchBand label for a combined score in [50, 74]', () => {
    renderRow({ score: { ...BASE_SCORE, combined: 60 }, pending: false, hasResume: true });
    expect(screen.getByText('Medium')).toBeInTheDocument();
  });

  it('renders the Low MatchBand label for a combined score < 50', () => {
    renderRow({ score: { ...BASE_SCORE, combined: 30 }, pending: false, hasResume: true });
    expect(screen.getByText('Low')).toBeInTheDocument();
  });

  it('prefers the score over the pending placeholder when both are set', () => {
    const { container } = renderRow({
      score: { ...BASE_SCORE, combined: 82 },
      pending: true,
      hasResume: true,
    });
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).not.toBeInTheDocument();
  });
});

describe('RowMatchScore — pending state', () => {
  it('shows the aria-busy loading placeholder while the batch is in-flight', () => {
    const { container } = renderRow({ pending: true, hasResume: true });

    const busyEl = container.querySelector('[aria-busy="true"]');
    expect(busyEl).toBeInTheDocument();
    expect(busyEl).toHaveTextContent('…');
  });

  it('loading placeholder carries the jobs.scoreLoading aria-label', () => {
    const { container } = renderRow({ pending: true, hasResume: true });

    expect(container.querySelector('[aria-busy="true"]')).toHaveAttribute(
      'aria-label',
      'jobs.scoreLoading'
    );
  });
});

describe('RowMatchScore — settled with no score', () => {
  it('renders nothing when the batch settled but this row has no score', () => {
    const { container } = renderRow({ pending: false, hasResume: true });
    expect(container.firstChild).toBeNull();
  });
});
