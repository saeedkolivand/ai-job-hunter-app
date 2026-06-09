/**
 * RowMatchScore — auto-fire + render-state tests.
 *
 * Covers:
 *  - Auto-fires api.match.resume on render when resumeId + jobId are both present
 *  - While isPending shows the aria-busy loading placeholder
 *  - When data resolves shows the MatchBand tier label (High/Medium/Low)
 *  - When resumeId is null (no documents) renders nothing and never calls the API
 *  - When the query rejects renders nothing (isError path)
 *  - When resolved data carries an error field renders nothing
 */
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/dom';
import { render, waitFor } from '@testing-library/react';

import type { MatchScore } from '@ajh/shared';

import { ScoringSchedulerProvider } from '@/providers/ScoringScheduler';
import { createMockClient, makeQueryClient, withProviders } from '@/test-support';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@/lib/i18n', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── useDocuments stub ─────────────────────────────────────────────────────────
// Module-level ref so each test sets it BEFORE render (same pattern as
// ReferralModal.test.tsx stubbedDraft). Never set after render.

let stubbedDocs: Array<{ _id: string; isDefault?: boolean }> = [];

vi.mock('@/services', async () => {
  const real = await import('@/services/use-match/use-match');
  return {
    useDocuments: () => ({ data: stubbedDocs }),
    // Real useJobMatchScore so we exercise the actual useQuery wiring.
    useJobMatchScore: real.useJobMatchScore,
  };
});

// ── component under test ──────────────────────────────────────────────────────

import { RowMatchScore } from './index';

// ── constants ─────────────────────────────────────────────────────────────────

const JOB_ID = 'job-abc';
const RESUME_ID = 'resume-xyz';
const ONE_DOC = [{ _id: RESUME_ID, isDefault: true }];

const BASE_SCORE: MatchScore = { combined: 82, semantic: 80, keyword: 84 };

// ── helper — always set stubbedDocs BEFORE calling render ─────────────────────

function renderScore(
  matchResumeFn: (...args: never[]) => unknown,
  docs: typeof stubbedDocs = ONE_DOC
) {
  stubbedDocs = docs;
  const client = createMockClient({ 'match.resume': matchResumeFn });
  const qc = makeQueryClient();
  const Base = withProviders(client, qc);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Base>
      <ScoringSchedulerProvider>{children}</ScoringSchedulerProvider>
    </Base>
  );
  const { container } = render(<RowMatchScore jobId={JOB_ID} />, { wrapper });
  return { container, matchResumeFn: matchResumeFn as ReturnType<typeof vi.fn> };
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe('RowMatchScore — auto-fires on render', () => {
  it('calls api.match.resume without any user interaction when resumeId and jobId are present', async () => {
    const fn = vi.fn().mockResolvedValue(BASE_SCORE);
    renderScore(fn);

    await waitFor(() => expect(fn).toHaveBeenCalledTimes(1));
    expect(fn).toHaveBeenCalledWith({
      resumeId: RESUME_ID,
      jobId: JOB_ID,
      semanticScoringEnabled: false,
    });
  });
});

describe('RowMatchScore — pending state', () => {
  it('shows the aria-busy loading placeholder while the query is in-flight', () => {
    // Never-resolving promise keeps the component in isPending.
    renderScore(vi.fn().mockReturnValue(new Promise(() => {})));

    const busyEl = document.querySelector('[aria-busy="true"]');
    expect(busyEl).toBeInTheDocument();
    expect(busyEl).toHaveTextContent('…');
  });

  it('loading placeholder carries the jobs.scoreLoading aria-label', () => {
    renderScore(vi.fn().mockReturnValue(new Promise(() => {})));

    expect(document.querySelector('[aria-busy="true"]')).toHaveAttribute(
      'aria-label',
      'jobs.scoreLoading'
    );
  });
});

describe('RowMatchScore — score state', () => {
  it('renders the High MatchBand label for a combined score >= 75', async () => {
    renderScore(vi.fn().mockResolvedValue({ ...BASE_SCORE, combined: 82 }));

    await waitFor(() => expect(screen.getByText('High')).toBeInTheDocument());
    expect(document.querySelector('[aria-busy="true"]')).not.toBeInTheDocument();
  });

  it('renders the Medium MatchBand label for a combined score in [50, 74]', async () => {
    renderScore(vi.fn().mockResolvedValue({ ...BASE_SCORE, combined: 60 }));

    await waitFor(() => expect(screen.getByText('Medium')).toBeInTheDocument());
  });

  it('renders the Low MatchBand label for a combined score < 50', async () => {
    renderScore(vi.fn().mockResolvedValue({ ...BASE_SCORE, combined: 30 }));

    await waitFor(() => expect(screen.getByText('Low')).toBeInTheDocument());
  });

  it('renders nothing when the resolved data carries an error field', async () => {
    const fn = vi
      .fn()
      .mockResolvedValue({ combined: 0, semantic: 0, keyword: 0, error: 'no embeddings' });
    const { container } = renderScore(fn);

    // Wait for the loading placeholder to disappear — that confirms the query
    // settled and the component re-rendered with the error-field result.
    await waitFor(() =>
      expect(document.querySelector('[aria-busy="true"]')).not.toBeInTheDocument()
    );
    expect(container.firstChild).toBeNull();
  });
});

describe('RowMatchScore — no resume state', () => {
  it('renders nothing and skips the API call when there are no documents', () => {
    // stubbedDocs = [] → useDefaultResumeId returns null → query disabled.
    const fn = vi.fn().mockResolvedValue(BASE_SCORE);
    const { container } = renderScore(fn, []);

    expect(container.firstChild).toBeNull();
    expect(fn).not.toHaveBeenCalled();
  });
});

describe('RowMatchScore — error state', () => {
  it('renders nothing when the query rejects', async () => {
    const fn = vi.fn().mockRejectedValue(new Error('network error'));
    const { container } = renderScore(fn);

    // Wait for loading placeholder to disappear — confirms the rejected promise
    // propagated and the component re-rendered into the isError → null branch.
    await waitFor(() =>
      expect(document.querySelector('[aria-busy="true"]')).not.toBeInTheDocument()
    );
    expect(container.firstChild).toBeNull();
  });
});
