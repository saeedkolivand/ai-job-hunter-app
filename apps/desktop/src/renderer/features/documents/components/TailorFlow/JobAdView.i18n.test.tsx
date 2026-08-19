/**
 * JobAdView — Score tab: real-i18n label guard + null-vs-zero rendering guard.
 *
 * Two absolute checks a raw-key-echo i18n mock (see JobAdView.test.tsx) can
 * never catch:
 *
 *  1. The tab never labels a number "ATS score". `MatchScore.ats` (deterministic
 *     keyword coverage) and the analyzer's `atsScore` (an LLM judgement) are two
 *     different engines — one shared label would make both read as one number.
 *  2. A metric that was not actually measured (semantic scoring off, or a
 *     posting with no extractable keywords) renders the real translated
 *     "not scored" copy, never a bare "0%" — a `0` there would be a placeholder
 *     dressed as a result.
 *
 * Deliberately does NOT mock `@ajh/translations` (same rationale as
 * match-band.i18n.test.tsx / trust-badge.test.tsx) — it resolves to the real
 * bundled en resources under vitest, so a mistyped/missing key, or an
 * accidental "ATS score" label, surfaces here instead of being hidden behind
 * `t: (k) => k`.
 *
 * `@/services`' `useJobMatchScore` IS stubbed — same pattern as
 * MatchScoresProvider.test.tsx — so no QueryClient/AppClient/IPC is needed.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { MatchScore } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import i18n from '@ajh/translations';

// ── useJobMatchScore stub ─────────────────────────────────────────────────────

let stubbedScore: { data?: MatchScore; isLoading?: boolean } = {};

vi.mock('@/services', () => ({
  useJobMatchScore: () => stubbedScore,
}));

// ── self-contained store-driven pickers — never mounted on the Score tab
// (every test here starts on `source`, see makeProps), stubbed only so the
// module import itself stays cheap and hermetic ─────────────────────────────

vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => null,
}));

vi.mock('@/lib/generate', () => ({
  OUTPUT_LANGUAGES: [{ code: 'en', endonym: 'English' }],
}));

// ── Import component AFTER all mocks ─────────────────────────────────────────

import { JobAdView } from './JobAdView';

// ── fixtures ──────────────────────────────────────────────────────────────────

const RESUME_ID = 'resume-1';
const JOB_ID = 'job-1';

function baseScore(overrides: Partial<MatchScore> = {}): MatchScore {
  return {
    resumeId: RESUME_ID,
    jobId: JOB_ID,
    ats: 60,
    semantic: 0,
    combined: 55,
    gaps: ['docker'],
    recommendations: [],
    scoreSource: 'keyword',
    ...overrides,
  };
}

function makeProps(overrides: Partial<Parameters<typeof JobAdView>[0]> = {}) {
  return {
    jobDesc: '',
    onJobDescChange: vi.fn(),
    summary: '',
    generating: false,
    error: null,
    onGenerateSummary: vi.fn(),
    language: 'en',
    onLanguageChange: vi.fn(),
    hasDesc: false, // defaults to the `source` tab — no summary toolbar to stub
    resumeId: RESUME_ID,
    jobId: JOB_ID,
    ...overrides,
  };
}

async function openScoreTab() {
  await userEvent.click(screen.getByText(i18n.t('autopilot.apply.jobAdView.scoreTab')));
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. The label never reads "ATS score"
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab labels never read "ATS score"', () => {
  it('the real translated coverage/match labels are not "ATS score"', () => {
    const matchLabel = i18n.t('autopilot.apply.jobAdView.score.matchLabel');
    const coverageLabel = i18n.t('autopilot.apply.jobAdView.score.coverageLabel');
    // Anchored to the actual bundled copy, not a derived comparison.
    expect(matchLabel.toLowerCase()).not.toContain('ats score');
    expect(coverageLabel.toLowerCase()).not.toContain('ats score');
    // A missing/mistyped key would echo the raw key back — rule that out too.
    expect(matchLabel).not.toBe('autopilot.apply.jobAdView.score.matchLabel');
    expect(coverageLabel).not.toBe('autopilot.apply.jobAdView.score.coverageLabel');
  });

  it('no rendered string on the Score tab reads "ATS score"', async () => {
    stubbedScore = { data: baseScore(), isLoading: false };
    const { container } = render(<JobAdView {...makeProps()} />);
    await openScoreTab();
    expect(container.textContent?.toLowerCase()).not.toContain('ats score');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. An unmeasured metric never renders a bare 0
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab never renders a 0 for something that was not measured', () => {
  it('semantic scoring off (scoreSource: keyword) shows real "not scored" copy, not "0%"', async () => {
    stubbedScore = { data: baseScore({ semantic: 0, scoreSource: 'keyword' }), isLoading: false };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    const notScored = i18n.t('analyze.notScored');
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreSemantic)).toHaveTextContent(
      notScored
    );
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });

  it('a posting with no extractable keywords (ats: 0, gaps: []) shows the real reason, not "0%"', async () => {
    stubbedScore = {
      data: baseScore({ ats: 0, gaps: [], combined: 0, scoreSource: 'keyword' }),
      isLoading: false,
    };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    const noKeywords = i18n.t('autopilot.apply.jobAdView.score.noKeywords');
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreCoverage)).toHaveTextContent(
      noKeywords
    );
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreMatch)).toHaveTextContent(
      noKeywords
    );
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });

  it('a genuine 0% coverage (job has keywords, none matched) DOES show "0%" — not suppressed', async () => {
    // Distinguishes the two 0-ats cases: real 0% still lists gaps.
    stubbedScore = {
      data: baseScore({ ats: 0, gaps: ['rust', 'docker'], combined: 0, scoreSource: 'keyword' }),
      isLoading: false,
    };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreCoverage)).toHaveTextContent('0%');
  });

  it('no stored résumé (resumeId undefined) shows the real reason, never a score', async () => {
    render(<JobAdView {...makeProps({ resumeId: undefined })} />);
    await openScoreTab();

    expect(screen.getByText(i18n.t('jobs.scoreNoResume'))).toBeInTheDocument();
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });

  it('no job to score against (jobId undefined) shows the real reason, never a score', async () => {
    render(<JobAdView {...makeProps({ jobId: undefined })} />);
    await openScoreTab();

    expect(screen.getByText(i18n.t('autopilot.apply.jobAdView.score.noJob'))).toBeInTheDocument();
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });
});
