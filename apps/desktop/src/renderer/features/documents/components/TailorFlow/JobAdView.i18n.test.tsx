/**
 * JobAdView — Score tab: real-i18n label guard + null-vs-zero rendering guard.
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
 * Deliberately does NOT mock `@ajh/translations` (same rationale as
 * match-band.i18n.test.tsx / trust-badge.test.tsx) — it resolves to the real
 * bundled en resources under vitest, so a mistyped/missing key, or an
 * accidental "ATS score" label, surfaces here instead of being hidden behind
 * `t: (k) => k`.
 *
 * `@/services`' `useJobAdTextMatchScore` IS stubbed — same pattern as
 * MatchScoresProvider.test.tsx — so no QueryClient/AppClient/IPC is needed.
 * It's a tracked `vi.fn` (not a plain arrow) so the resumeId-threading test
 * below can assert on its call arguments. The keystroke-storm guard (editing
 * the posting text must not re-fire the query) is covered separately, against
 * the same tracked-mock pattern, in JobAdView.test.tsx §10.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { MatchScore } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import i18n from '@ajh/translations';

// ── useJobAdTextMatchScore stub ───────────────────────────────────────────────

let stubbedScore: {
  data?: MatchScore;
  isLoading?: boolean;
  isError?: boolean;
  refetch?: () => void;
} = {};

// Reset before EVERY test, not just the ones that assign it explicitly.
// Three tests below (the no-résumé / empty-posting / whitespace-only-posting
// cases) never set `stubbedScore` at all — they pass because the component
// short-circuits on `!resumeId`/`!scoreText` before ever reading the score,
// but without this reset they'd silently inherit whatever the PREVIOUS test
// left behind, which only stays harmless as long as test order and every
// other test's cleanup happen to line up.
beforeEach(() => {
  stubbedScore = {};
});

const mockUseJobAdTextMatchScore = vi.fn(
  (_resumeId: string | null, _jobText: string, _enabled?: boolean) => stubbedScore
);

vi.mock('@/services', () => ({
  useJobAdTextMatchScore: (...args: Parameters<typeof mockUseJobAdTextMatchScore>) =>
    mockUseJobAdTextMatchScore(...args),
}));

// ── self-contained store-driven pickers — never mounted on the Score tab
// (every test here starts on `source`, see makeProps), stubbed only so the
// module import itself stays cheap and hermetic ─────────────────────────────

// Mutable so the CLI-agent egress-disclosure tests below can select a
// CLI-agent provider; every other test in this file never touches it, so it
// stays at the default ('ollama', kind: local-server — no disclosure).
let mockActiveProvider = 'ollama';

vi.mock('@/components/ui/ModelSelector', () => ({
  ModelSelector: () => null,
  useSelectedProvider: () => mockActiveProvider,
}));

vi.mock('@/lib/generate', () => ({
  OUTPUT_LANGUAGES: [{ code: 'en', endonym: 'English' }],
}));

// ── Import component AFTER all mocks ─────────────────────────────────────────

import { JobAdView } from './JobAdView';

// ── fixtures ──────────────────────────────────────────────────────────────────

const RESUME_ID = 'resume-1';
const JOB_ID = 'job-1';

/** `combined === ats` whenever `scoreSource: 'keyword'` — the kernel's own
 *  invariant (`match_resume.rs`: `combined` only diverges from `ats` when a
 *  semantic comparison actually ran, which sets `scoreSource: 'combined'`).
 *  A fixture that violates this (e.g. `ats: 60, combined: 55` under
 *  `'keyword'`) is one `match:text` can never actually produce. */
function baseScore(overrides: Partial<MatchScore> = {}): MatchScore {
  return {
    resumeId: RESUME_ID,
    jobId: JOB_ID,
    ats: 60,
    semantic: 0,
    combined: 60,
    gaps: ['docker'],
    recommendations: [],
    scoreSource: 'keyword',
    ...overrides,
  };
}

function makeProps(overrides: Partial<Parameters<typeof JobAdView>[0]> = {}) {
  return {
    // Non-empty by default — the Score tab snapshots this text the instant it
    // opens, and an empty snapshot renders the "no posting" reason instead of
    // the stubbed score (see the "posting text is empty" test below, which
    // overrides this back to '').
    jobDesc: 'A job posting with enough text to score against.',
    onJobDescChange: vi.fn(),
    summary: '',
    generating: false,
    error: null,
    onGenerateSummary: vi.fn(),
    language: 'en',
    onLanguageChange: vi.fn(),
    hasDesc: false, // defaults to the `source` tab — no summary toolbar to stub
    resumeId: RESUME_ID,
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
    stubbedScore = { data: baseScore({ scoreSource: 'combined' }), isLoading: false };
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
    // Match is dropped entirely for a keyword-only score — see block 5 below.
    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewScoreMatch)).not.toBeInTheDocument();
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

  it('an empty posting (blank snapshot) shows the real reason, never a score', async () => {
    render(<JobAdView {...makeProps({ jobDesc: '' })} />);
    await openScoreTab();

    expect(
      screen.getByText(i18n.t('autopilot.apply.jobAdView.score.noPosting'))
    ).toBeInTheDocument();
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });

  it('a whitespace-only posting is treated as empty, never a score', async () => {
    render(<JobAdView {...makeProps({ jobDesc: '   ' })} />);
    await openScoreTab();

    expect(
      screen.getByText(i18n.t('autopilot.apply.jobAdView.score.noPosting'))
    ).toBeInTheDocument();
    expect(screen.queryByText('0%')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3. A failed request is never laundered into "not scored"
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: a failed request renders a distinct error', () => {
  it('isError renders the translated error copy, never the not-scored placeholder', async () => {
    stubbedScore = { data: undefined, isLoading: false, isError: true, refetch: vi.fn() };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(
      screen.getByText(i18n.t('autopilot.apply.jobAdView.score.errorTitle'))
    ).toBeInTheDocument();
    expect(screen.queryByText(i18n.t('analyze.notScored'))).not.toBeInTheDocument();
  });

  it("retrying calls the query's own refetch — not a page reload or a re-derived request", async () => {
    const refetch = vi.fn();
    stubbedScore = { data: undefined, isLoading: false, isError: true, refetch };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

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
    stubbedScore = {
      data: { resumeId: RESUME_ID, jobId: JOB_ID } as unknown as MatchScore,
      isLoading: false,
      refetch: vi.fn(),
    };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(screen.queryByText(/NaN%/)).not.toBeInTheDocument();
    expect(screen.queryByText(i18n.t('jobs.matchBand.Low'))).not.toBeInTheDocument();
    expect(
      screen.getByText(i18n.t('autopilot.apply.jobAdView.score.errorTitle'))
    ).toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4. Loading is announced to screen readers
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: loading is a live region', () => {
  it('has role=status + aria-live=polite — the one state most needing it (a translating call can take ~117s)', async () => {
    stubbedScore = { data: undefined, isLoading: true, isError: false, refetch: vi.fn() };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-live', 'polite');
    expect(status).toHaveTextContent(i18n.t('autopilot.apply.jobAdView.score.loading'));
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
    stubbedScore = {
      data: baseScore({ ats: 60, combined: 60, scoreSource: 'keyword' }),
      isLoading: false,
    };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewScoreMatch)).not.toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreCoverage)).toHaveTextContent('60%');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 6. resumeId reaches the leaf hook call (the exact defect shape that left
//    `jobId` dead at every call site since the commit that introduced it)
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: resumeId threading', () => {
  it('the resumeId prop is the argument the scoring hook is actually called with', async () => {
    // Earlier tests in this file also open the Score tab (with the default
    // RESUME_ID) — clear so `.find` below can't pick up a stale call.
    mockUseJobAdTextMatchScore.mockClear();
    stubbedScore = { data: baseScore(), isLoading: false };
    render(<JobAdView {...makeProps({ resumeId: 'resume-xyz' })} />);
    await openScoreTab();

    const enabledCall = mockUseJobAdTextMatchScore.mock.calls.find((call) => call[2] === true);
    expect(enabledCall?.[0]).toBe('resume-xyz');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 7. Additional context the panel now surfaces: which résumé, the keyword
//    count behind coverage, and the top missing keywords
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: additional context lines', () => {
  it('states which résumé the score is against, so it is never read as the tailored doc shown one view over', async () => {
    stubbedScore = { data: baseScore(), isLoading: false };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(
      screen.getByText(i18n.t('autopilot.apply.jobAdView.score.resumeNote'))
    ).toBeInTheDocument();
  });

  it('renders the kernel explanation with the echoed guidance sentence trimmed (never duplicated)', async () => {
    // The real Rust GUIDANCE constant (match_resume.rs) — deliberately the
    // exact string `jobs.scoreGuidance` already renders at the top of this
    // panel, so a failed trim would make it appear twice.
    const guidance =
      "This score is a guidance estimate — not the employer's decision or any ATS system's score.";
    stubbedScore = {
      data: baseScore({
        ats: 47,
        combined: 47,
        scoreSource: 'keyword',
        explanation: `Keyword coverage 47% across 312 job keywords (semantic scoring disabled). ${guidance}`,
        guidance,
      }),
      isLoading: false,
    };
    const { container } = render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(screen.getByText(/312 job keywords/)).toBeInTheDocument();
    const occurrences = (
      container.textContent?.match(/employer's decision or any ATS system's score/g) ?? []
    ).length;
    expect(occurrences).toBe(1);
  });

  it('renders up to 3 missing keywords as chips, capped even when more are present', async () => {
    stubbedScore = {
      data: baseScore({
        ats: 40,
        combined: 40,
        scoreSource: 'keyword',
        gaps: ['docker', 'kubernetes', 'terraform', 'aws'],
      }),
      isLoading: false,
    };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(screen.getByText(i18n.t('analyze.gaps'))).toBeInTheDocument();
    expect(screen.getByText('docker')).toBeInTheDocument();
    expect(screen.getByText('kubernetes')).toBeInTheDocument();
    expect(screen.getByText('terraform')).toBeInTheDocument();
    expect(screen.queryByText('aws')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 8. CLI-agent egress disclosure — a CLI-agent provider (Claude Code, Codex,
// Gemini CLI) egresses despite reading as "local" elsewhere in this app; the
// translation path a foreign-language posting routes through can send it the
// job ad text. Undisclosed, this surface would say nothing about it.
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: CLI-agent egress disclosure', () => {
  afterEach(() => {
    mockActiveProvider = 'ollama';
  });

  it('discloses egress, with the provider named, when the active provider is a CLI agent', async () => {
    mockActiveProvider = 'claude-code';
    stubbedScore = { data: baseScore(), isLoading: false };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    expect(
      screen.getByText(
        i18n.t('autopilot.apply.jobAdView.score.cliAgentEgress', { provider: 'Claude Code' })
      )
    ).toBeInTheDocument();
  });

  it('does NOT disclose egress for a local provider (ollama)', async () => {
    mockActiveProvider = 'ollama';
    stubbedScore = { data: baseScore(), isLoading: false };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    // Anchored on the untranslated key resolving to an actual sentence, so a
    // false negative here can't hide behind a missing-key echo.
    const egress = i18n.t('autopilot.apply.jobAdView.score.cliAgentEgress', {
      provider: 'Ollama (Local)',
    });
    expect(egress).not.toBe('autopilot.apply.jobAdView.score.cliAgentEgress');
    expect(screen.queryByText(egress)).not.toBeInTheDocument();
  });

  it('does NOT disclose egress for a cloud API provider (openai) — only CLI agents egress unexpectedly', async () => {
    // Cloud providers already read as external everywhere else in the app;
    // this disclosure exists specifically because CLI agents are the ones
    // that WRONGLY read as local (`is_local()`) despite egressing.
    mockActiveProvider = 'openai';
    stubbedScore = { data: baseScore(), isLoading: false };
    render(<JobAdView {...makeProps()} />);
    await openScoreTab();

    const egress = i18n.t('autopilot.apply.jobAdView.score.cliAgentEgress', {
      provider: 'OpenAI',
    });
    expect(screen.queryByText(egress)).not.toBeInTheDocument();
  });
});
