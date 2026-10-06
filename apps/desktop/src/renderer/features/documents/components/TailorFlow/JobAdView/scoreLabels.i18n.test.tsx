/**
 * JobAdView — Score tab (real en copy): label + null-vs-zero rendering guard.
 *
 * Absolute checks a raw-key-echo i18n mock (see JobAdView.test.tsx) can never
 * catch:
 *
 *  1. The tab never labels a number "ATS score". `MatchScore.ats` (deterministic
 *     keyword coverage) and the analyzer's `atsScore` (an LLM judgement) are two
 *     different engines — one shared label would make both read as one number.
 *  2. A metric that was not actually measured (semantic scoring off, or a
 *     posting with no extractable keywords) renders the real translated
 *     "not scored" copy, never a bare "0%" — a `0` there would be a placeholder
 *     dressed as a result.
 *  3. A failed request renders a distinct, translated error — never the same
 *     "not scored" copy a legitimate empty state uses.
 *  4. A malformed (but resolved) IPC response never renders `NaN%` under a
 *     red "Low" badge.
 *  5. `scoreSource: 'keyword'` (the only value this endpoint ever returns)
 *     never renders the Match row alongside Coverage — same number, two
 *     contradictory badge cut points.
 *
 * Shared stubs live in `i18n-support.tsx` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { MatchScore } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import i18n from '@ajh/translations';

import { renderScoreTab } from './i18n-helpers';
import { JOB_ID, resetStub, RESUME_ID, setScore, stub } from './i18n-support';

vi.mock('@/services', async () => (await import('./i18n-support')).servicesModule);
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./i18n-support')).modelSelectorModule;
});
vi.mock('@/lib/generate', async () => (await import('./i18n-support')).generateModule);

beforeEach(resetStub);

const T = (key: string) => i18n.t(`autopilot.apply.jobAdView.score.${key}`);
const noZero = () => expect(screen.queryByText('0%')).not.toBeInTheDocument();
const coverage = () => screen.getByTestId(TEST_IDS.documents.jobAdViewScoreCoverage);
const matchRow = () => screen.queryByTestId(TEST_IDS.documents.jobAdViewScoreMatch);

// ─────────────────────────────────────────────────────────────────────────────
// 1. The label never reads "ATS score"
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab labels never read "ATS score"', () => {
  it('the real translated coverage/match labels are not "ATS score"', () => {
    // Reused from the Autopilot list's own field labels — see the "one
    // engine, one label" test below for why these are NOT re-forked here.
    const matchLabel = i18n.t('autopilot.scoreAbbr.combined');
    const coverageLabel = i18n.t('autopilot.scoreAbbr.coverage');
    // Anchored to the actual bundled copy, not a derived comparison.
    expect(matchLabel.toLowerCase()).not.toContain('ats score');
    expect(coverageLabel.toLowerCase()).not.toContain('ats score');
    // A missing/mistyped key would echo the raw key back — rule that out too.
    expect(matchLabel).not.toBe('autopilot.scoreAbbr.combined');
    expect(coverageLabel).not.toBe('autopilot.scoreAbbr.coverage');
  });

  it('no rendered string on the Score tab reads "ATS score"', async () => {
    setScore({ scoreSource: 'combined' });
    const { container } = await renderScoreTab();
    expect(container.textContent?.toLowerCase()).not.toContain('ats score');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2. An unmeasured metric never renders a bare 0
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab never renders a 0 for something that was not measured', () => {
  it('semantic scoring off (scoreSource: keyword) shows real "not scored" copy, not "0%"', async () => {
    setScore({ semantic: 0, scoreSource: 'keyword' });
    await renderScoreTab();

    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreSemantic)).toHaveTextContent(
      i18n.t('analyze.notScored')
    );
    noZero();
  });

  it('a posting with no extractable keywords (ats: 0, gaps: []) shows the real reason, not "0%"', async () => {
    setScore({ ats: 0, gaps: [], combined: 0, scoreSource: 'keyword' });
    await renderScoreTab();

    expect(coverage()).toHaveTextContent(T('noKeywords'));
    // Match is dropped entirely for a keyword-only score — see block 5 below.
    expect(matchRow()).not.toBeInTheDocument();
    noZero();
  });

  it('a genuine 0% coverage (job has keywords, none matched) DOES show "0%" — not suppressed', async () => {
    // Distinguishes the two 0-ats cases: real 0% still lists gaps.
    setScore({ ats: 0, gaps: ['rust', 'docker'], combined: 0, scoreSource: 'keyword' });
    await renderScoreTab();

    expect(coverage()).toHaveTextContent('0%');
  });

  it.each([
    [
      'no stored résumé (resumeId undefined)',
      { resumeId: undefined },
      i18n.t('jobs.scoreNoResume'),
    ],
    ['an empty posting (blank snapshot)', { jobDesc: '' }, T('noPosting')],
    ['a whitespace-only posting (treated as empty)', { jobDesc: '   ' }, T('noPosting')],
  ])('%s shows the real reason, never a score', async (_name, overrides, reason) => {
    await renderScoreTab(overrides);

    expect(screen.getByText(reason)).toBeInTheDocument();
    noZero();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. A failed request is never laundered into "not scored"
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: a failed request renders a distinct error', () => {
  it('isError renders the translated error copy, never the not-scored placeholder', async () => {
    stub.score = { data: undefined, isLoading: false, isError: true, refetch: vi.fn() };
    await renderScoreTab();

    expect(screen.getByText(T('errorTitle'))).toBeInTheDocument();
    expect(screen.queryByText(i18n.t('analyze.notScored'))).not.toBeInTheDocument();
  });

  it("retrying calls the query's own refetch — not a page reload or a re-derived request", async () => {
    const refetch = vi.fn();
    stub.score = { data: undefined, isLoading: false, isError: true, refetch };
    await renderScoreTab();

    // `ErrorState`'s retry button label ("Try again") is a fixed string in the
    // shared primitive itself, not a translation key of this component's.
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it('a malformed-but-resolved response (ats/combined not numbers) renders the SAME error state, never NaN% or a red Low badge', async () => {
    // `match_resume_text` can resolve `{ error: "resume not found: …" }`
    // instead of a MatchScore — `invoke()` does not validate the resolved
    // shape, so a deleted résumé (the wizard's `resumeDocId` can outlive the
    // document it points at — the in-wizard delete path clears it, the
    // out-of-wizard one does not) reaches this component typed as MatchScore
    // while actually missing `ats`/`combined`. The cast mirrors that real
    // gap: TypeScript can't catch it, only a runtime guard can.
    stub.score = {
      data: { resumeId: RESUME_ID, jobId: JOB_ID } as unknown as MatchScore,
      isLoading: false,
      refetch: vi.fn(),
    };
    await renderScoreTab();

    expect(screen.queryByText(/NaN%/)).not.toBeInTheDocument();
    expect(screen.queryByText(i18n.t('jobs.matchBand.Low'))).not.toBeInTheDocument();
    expect(screen.getByText(T('errorTitle'))).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. Loading is announced to screen readers
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: loading is a live region', () => {
  it('has role=status + aria-live=polite — the one state most needing it (a translating call can take ~117s)', async () => {
    stub.score = { data: undefined, isLoading: true, isError: false, refetch: vi.fn() };
    await renderScoreTab();

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-live', 'polite');
    expect(status).toHaveTextContent(T('loading'));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5. Match never rides alongside Coverage on an identical, keyword-only number
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: the Match row is dropped for a keyword-only score', () => {
  it('renders ONLY Coverage when scoreSource is keyword — never two bands on one number', async () => {
    // `combined === ats` here (the kernel invariant) — the fixture the OLD
    // test used (`ats: 60, combined: 55`) was one `match:text` can never
    // produce; this one is, and it's exactly the case that used to print
    // "Match 60% MEDIUM" directly above "Keyword coverage 60% HIGH".
    setScore({ ats: 60, combined: 60, scoreSource: 'keyword' });
    await renderScoreTab();

    expect(matchRow()).not.toBeInTheDocument();
    expect(coverage()).toHaveTextContent('60%');
  });
});
