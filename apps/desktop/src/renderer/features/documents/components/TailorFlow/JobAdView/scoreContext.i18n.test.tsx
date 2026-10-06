/**
 * JobAdView — Score tab (real en copy): resumeId threading, context lines,
 * recommendations, and the semantic-scoring states.
 *
 * Shared stubs live in `i18n-support.tsx` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';
import i18n from '@ajh/translations';

import { renderScoreTab } from './i18n-helpers';
import { mockUseJobAdTextMatchScore, resetStub, setScore } from './i18n-support';

vi.mock('@/services', async () => (await import('./i18n-support')).servicesModule);
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./i18n-support')).modelSelectorModule;
});
vi.mock('@/lib/generate', async () => (await import('./i18n-support')).generateModule);

beforeEach(resetStub);

// ─────────────────────────────────────────────────────────────────────────────
// 6. resumeId reaches the leaf hook call (the exact defect shape that left
//    `jobId` dead at every call site since the commit that introduced it)
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: resumeId threading', () => {
  it('the resumeId prop is the argument the scoring hook is actually called with', async () => {
    // Earlier tests in this file also open the Score tab (with the default
    // RESUME_ID) — clear so `.find` below can't pick up a stale call.
    mockUseJobAdTextMatchScore.mockClear();
    setScore();
    await renderScoreTab({ resumeId: 'resume-xyz' });

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
    setScore();
    await renderScoreTab();

    expect(
      screen.getByText(i18n.t('autopilot.apply.jobAdView.score.resumeNote'))
    ).toBeInTheDocument();
  });

  it('renders the kernel explanation with the echoed guidance sentence trimmed (never duplicated)', async () => {
    // The real Rust GUIDANCE constant (`match_resume/score.rs`) — deliberately the
    // exact string `jobs.scoreGuidance` already renders at the top of this
    // panel, so a failed trim would make it appear twice.
    const guidance =
      "This score is a guidance estimate — not the employer's decision or any ATS system's score.";
    setScore({
      ats: 47,
      combined: 47,
      scoreSource: 'keyword',
      explanation: `Keyword coverage 47% across 312 job keywords (semantic scoring disabled). ${guidance}`,
      guidance,
    });
    const { container } = await renderScoreTab();

    expect(screen.getByText(/312 job keywords/)).toBeInTheDocument();
    const occurrences = (
      container.textContent?.match(/employer's decision or any ATS system's score/g) ?? []
    ).length;
    expect(occurrences).toBe(1);
  });

  it('renders up to 3 missing keywords as chips, capped even when more are present', async () => {
    setScore({
      ats: 40,
      combined: 40,
      scoreSource: 'keyword',
      gaps: ['docker', 'kubernetes', 'terraform', 'aws'],
    });
    await renderScoreTab();

    expect(screen.getByText(i18n.t('analyze.gaps'))).toBeInTheDocument();
    expect(screen.getByText('docker')).toBeInTheDocument();
    expect(screen.getByText('kubernetes')).toBeInTheDocument();
    expect(screen.getByText('terraform')).toBeInTheDocument();
    expect(screen.queryByText('aws')).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 9. Recommendations — `MatchScore.recommendations` (backend-authored prose,
// like `explanationText`) surfaced below the gaps chips. Gated on
// `hasCoverage`: the "no extractable keywords" placeholder result still runs
// the same `recommendations(&gaps)` fn over its empty `gaps`, which would
// otherwise print a false "Strong keyword coverage" for a posting that was
// never actually scored.
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: recommendations', () => {
  it.each([
    [
      'renders the backend-authored recommendation prose under the Recommendations heading',
      {
        ats: 40,
        combined: 40,
        gaps: ['docker'],
        recommendations: ['Consider adding evidence of: docker.'],
      },
      'Consider adding evidence of: docker.',
      true,
    ],
    [
      'renders a positive recommendation for a strong-coverage score with no gaps',
      {
        ats: 95,
        combined: 95,
        gaps: [],
        recommendations: ['Strong keyword coverage — no obvious gaps.'],
      },
      'Strong keyword coverage — no obvious gaps.',
      false,
    ],
  ])('%s', async (_name, overrides, prose, checksHeading) => {
    setScore({ scoreSource: 'keyword', ...overrides });
    await renderScoreTab();

    if (checksHeading)
      expect(screen.getByText(i18n.t('analyze.recommendations'))).toBeInTheDocument();
    expect(screen.getByText(prose)).toBeInTheDocument();
  });

  it('does NOT render recommendations for the "no extractable keywords" placeholder, even if the field is non-empty', async () => {
    // `ats: 0, gaps: []` is the placeholder shape (see `hasCoverage`'s doc) —
    // the Rust `recommendations()` fn still runs over the empty `gaps` and
    // would produce "Strong keyword coverage — no obvious gaps.", which
    // would be a lie about a posting that was never actually scored.
    setScore({
      ats: 0,
      combined: 0,
      gaps: [],
      scoreSource: 'keyword',
      recommendations: ['Strong keyword coverage — no obvious gaps.'],
    });
    await renderScoreTab();

    expect(screen.queryByText(i18n.t('analyze.recommendations'))).not.toBeInTheDocument();
    expect(
      screen.queryByText('Strong keyword coverage — no obvious gaps.')
    ).not.toBeInTheDocument();
  });

  it('renders nothing under the heading when recommendations is empty', async () => {
    setScore({ recommendations: [] });
    await renderScoreTab();

    expect(screen.queryByText(i18n.t('analyze.recommendations'))).not.toBeInTheDocument();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 10. `hasSemantic`'s three reachable states, now that `match:text` is gated on
// the `semanticScoring` preference instead of hardcoded off. `scoreSource` is
// the ONLY signal the panel has (the hook itself decides what to request, per
// use-match.test.ts) — `'combined'` means it ran, `'keyword'` covers BOTH "off
// by preference" and "requested but degraded", distinguished only by the
// kernel's own `explanation` sentence.
// ─────────────────────────────────────────────────────────────────────────────

describe('JobAdView — Score tab: the three semantic-scoring states', () => {
  const semantic = () => screen.getByTestId(TEST_IDS.documents.jobAdViewScoreSemantic);

  it('semantic ran (scoreSource: combined) renders BOTH the Match row and a real Semantic percentage', async () => {
    setScore({ ats: 60, semantic: 80, combined: 74, scoreSource: 'combined' });
    await renderScoreTab();

    expect(screen.getByTestId(TEST_IDS.documents.jobAdViewScoreMatch)).toHaveTextContent('74%');
    expect(semantic()).toHaveTextContent('80%');
    expect(semantic()).not.toHaveTextContent(i18n.t('analyze.notScored'));
  });

  // The second case is the kernel's own degrade sentence (`score_one`'s explanation
  // branch for semantic_enabled=1 with no usable embedding pair) — not a fixture
  // invented for this test. Same honest footnote as the "off by preference" case:
  // the panel never fabricates a distinction it can't back with a number — but the
  // reason IS surfaced, not silently dropped, right below the footnote.
  it.each([
    [
      'semantic off by preference (scoreSource: keyword, "disabled" explanation) shows the honest not-scored footnote',
      'semantic scoring disabled',
      /semantic scoring disabled/,
    ],
    [
      'semantic requested but degraded (scoreSource: keyword, "could not be computed" explanation) still surfaces the real reason — never silently dropped',
      'semantic similarity could not be computed — no embedding was available for this pair',
      /semantic similarity could not be computed — no embedding was available/,
    ],
  ])('%s', async (_name, reason, matcher) => {
    const guidance =
      "This score is a guidance estimate — not the employer's decision or any ATS system's score.";
    setScore({
      ats: 60,
      combined: 60,
      scoreSource: 'keyword',
      explanation: `Keyword coverage 60% across 40 job keywords (${reason}). ${guidance}`,
      guidance,
    });
    await renderScoreTab();

    expect(screen.queryByTestId(TEST_IDS.documents.jobAdViewScoreMatch)).not.toBeInTheDocument();
    expect(semantic()).toHaveTextContent(i18n.t('analyze.notScored'));
    expect(screen.getByText(matcher)).toBeInTheDocument();
  });
});
