/**
 * GenerationOutput — quality badge (seeded report + staleness) and the résumé Score strip.
 * Mocks, props builder and helpers live in `harness.tsx` (see its header).
 */
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { MatchScore } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import { hashText, type QualityReport } from '@/lib/generate';

import {
  clickJobAdTab,
  GenerationOutput,
  makeProps,
  mockUseJobAdTextMatchScore,
  renderOutput,
  resetHarness,
  stub,
} from './harness';

beforeEach(resetHarness);

describe('GenerationOutput', () => {
  // ── Quality badge — cold-hydrated report + staleness (Phase-1 finding #3) ────
  // `output` (the doc GenerationOutput actually renders) is what a cold-entry
  // hydration seeds into the session's resumeOut/coverOut from a persisted
  // record's `resumeText`/`coverLetterText` — the same string a real
  // `parseQualityReport(seedGeneration.qualityReport)` was hashed against at
  // save time. This exercises the REAL QualityBadge (no stub), the level the
  // staleness comparison actually renders at.
  describe('quality badge — seeded report + staleness', () => {
    const OUTPUT = 'Generated resume content'; // matches makeProps()'s default `output`
    const PAYLOAD = {
      ok: true,
      issues: [],
      metrics: {
        keywordCoverage: 80,
        topRequirementHits: 1,
        duplicateRatio: 0,
        rolesSource: 1,
        rolesOutput: 1,
      },
    };
    const REPORT: QualityReport = {
      schemaVersion: 2,
      pipeline: 'fast',
      generatedAt: 1,
      resume: { report: PAYLOAD, sourceTextHash: hashText(OUTPUT) },
    };
    const STALE: QualityReport = {
      ...REPORT,
      resume: { report: PAYLOAD, sourceTextHash: hashText('DIFFERENT') },
    };

    it('renders the badge for a seeded, unedited report (hash matches — not stale)', () => {
      renderOutput({ report: REPORT });
      expect(screen.getByRole('button', { name: /quality\.badge\.clean/ })).toBeInTheDocument();
    });

    it('renders the stale state once the hash no longer matches (edited since hydration)', () => {
      renderOutput({ report: STALE });
      expect(screen.getByRole('button', { name: /quality\.badge\.stale/ })).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /quality\.badge\.clean/ })).toBeNull();
    });

    it('renders nothing when there is no report yet', () => {
      renderOutput({ report: null });
      expect(screen.queryByRole('button', { name: /quality\.badge/ })).toBeNull();
    });

    // This is the ONE surface with inline editing, so it is the only one whose
    // badge can go stale mid-session — it must also offer the way back out.
    it('offers Re-check in the panel when the host wires it', async () => {
      const user = userEvent.setup();
      const onRecheck = vi.fn();
      renderOutput({ report: STALE, onRecheck });

      await user.click(screen.getByRole('button', { name: /quality\.badge\.stale/ }));
      await user.click(screen.getByRole('button', { name: /quality\.panel\.recheck/ }));
      expect(onRecheck).toHaveBeenCalledTimes(1);
    });

    it('hides Re-check when the host cannot supply it', async () => {
      const user = userEvent.setup();
      renderOutput({ report: STALE });

      await user.click(screen.getByRole('button', { name: /quality\.badge\.stale/ }));
      expect(screen.queryByRole('button', { name: /quality\.panel\.recheck/ })).toBeNull();
    });
  });

  // ── 11. Score strip — résumé result surfaces the job-match score ─────────────
  // Real render-logic guards (isMeasured/hasScoreCoverage/ScoreMetric) are
  // covered once, at the source, by JobAdView/scoreEgress.i18n.test.tsx against REAL
  // translated copy — that module is now shared (MatchScoreMetric.tsx), not
  // forked. This block covers GenerationScoreStrip's OWN wiring: which tab it
  // renders on, and that it never fabricates a `0`.

  describe('Score strip', () => {
    // Clears accumulated calls from every earlier test in this file so the
    // "which text did the LATEST render call the hook with" test below finds
    // this test's own call, not some earlier test's.
    beforeEach(() => {
      mockUseJobAdTextMatchScore.mockClear();
    });

    function baseScore(overrides: Partial<MatchScore> = {}): MatchScore {
      return {
        resumeId: 'resume-1',
        jobId: 'job-1',
        ats: 72,
        semantic: 0,
        combined: 72,
        gaps: ['docker'],
        recommendations: [],
        scoreSource: 'keyword',
        ...overrides,
      };
    }
    const setScore = (overrides: Partial<MatchScore> = {}) => {
      stub.score = { data: baseScore(overrides), isLoading: false };
    };
    const renderStrip = (overrides: Parameters<typeof renderOutput>[0] = {}) =>
      renderOutput({ activeOut: 'resume', resumeId: 'resume-1', ...overrides });
    const strip = () => screen.getByTestId(TEST_IDS.documents.scoreStrip);
    const stripQuery = () => screen.queryByTestId(TEST_IDS.documents.scoreStrip);
    const E = (key: string) => `autopilot.apply.jobAdView.score.${key}`;
    const expectNoFabricatedScore = () => {
      expect(screen.queryByText('0%')).not.toBeInTheDocument();
      expect(screen.queryByText(/NaN%/)).not.toBeInTheDocument();
    };

    it('renders a real percentage when the score is measured', () => {
      setScore({ ats: 72 });
      renderStrip();
      expect(screen.getByTestId(TEST_IDS.documents.scoreStripCoverage)).toHaveTextContent('72%');
    });

    it('renders the stated reason, never "0%", for the no-extractable-keywords placeholder', () => {
      setScore({ ats: 0, combined: 0, gaps: [] });
      renderStrip();
      expect(screen.getByTestId(TEST_IDS.documents.scoreStripCoverage)).toHaveTextContent(
        E('noKeywords')
      );
      expect(screen.queryByText('0%')).not.toBeInTheDocument();
    });

    it('shows the no-résumé reason (never a score) when no resumeId is threaded', () => {
      renderStrip({ resumeId: undefined });
      expect(strip()).toHaveTextContent('jobs.scoreNoResume');
      expect(screen.queryByTestId(TEST_IDS.documents.scoreStripCoverage)).not.toBeInTheDocument();
    });

    it('does NOT render on the cover-letter tab — a cover letter is not scored against keyword coverage', () => {
      setScore();
      renderStrip({ target: 'both', activeOut: 'cover' });
      expect(stripQuery()).not.toBeInTheDocument();
    });

    it('does NOT render on the job-ad tab', async () => {
      const user = userEvent.setup();
      setScore();
      renderStrip();

      await clickJobAdTab(user);

      expect(stripQuery()).not.toBeInTheDocument();
    });

    it('passes the snapshotted jobDesc (not a live-editable value) as the query text argument', () => {
      setScore();
      renderStrip({ jobDesc: 'Snapshot-worthy posting text' });
      const enabledCall = mockUseJobAdTextMatchScore.mock.calls.find((call) => call[2] === true);
      expect(enabledCall?.[0]).toBe('resume-1');
      expect(enabledCall?.[1]).toBe('Snapshot-worthy posting text');
    });

    // ── Loading / error / malformed-payload branches ─────────────────────────
    // These are the branches that enforce the honesty guarantee itself (never a
    // fabricated `0%`/score for a request that hasn't resolved, failed, or came
    // back malformed).

    it('announces loading via a live region', () => {
      stub.score = { data: undefined, isLoading: true };
      renderStrip();

      expect(strip()).toHaveAttribute('role', 'status');
      expect(strip()).toHaveAttribute('aria-live', 'polite');
      expect(strip()).toHaveTextContent(E('loading'));
    });

    it('shows an alert with a working retry on a rejected request', async () => {
      const user = userEvent.setup();
      const refetch = vi.fn();
      stub.score = { data: undefined, isLoading: false, isError: true, refetch };
      renderStrip();

      expect(strip()).toHaveAttribute('role', 'alert');
      expect(strip()).toHaveTextContent(E('errorTitle'));

      await user.click(screen.getByRole('button', { name: /autopilot\.apply\.tryAgain/i }));
      expect(refetch).toHaveBeenCalledTimes(1);
    });

    it('renders the same alert (never a fabricated score) for a resolved-but-malformed payload', () => {
      // `invoke()` never validates the resolved shape — a failure response
      // (e.g. a résumé id that outlived its résumé) can resolve typed as
      // MatchScore while missing ats/combined/gaps/recommendations.
      stub.score = { data: { resumeId: 'resume-1', jobId: 'job-1' }, isLoading: false };
      renderStrip();

      expect(strip()).toHaveAttribute('role', 'alert');
      expect(strip()).toHaveTextContent(E('errorTitle'));
      expectNoFabricatedScore();
    });

    // ── CLI-agent egress disclosure (mirrors JobAdView's Score tab) ──────────
    // Unlike the Score tab, this strip fires ON MOUNT whenever a résumé is
    // threaded — no explicit click required — so the disclosure matters even
    // more here.

    it('discloses CLI-agent egress while loading and on the error branch, never for a local provider', () => {
      stub.provider = 'claude-code';
      stub.score = { data: undefined, isLoading: true };
      const { rerender } = renderStrip();
      expect(strip()).toHaveTextContent(E('cliAgentEgress'));

      stub.score = { data: undefined, isLoading: false, isError: true, refetch: vi.fn() };
      rerender(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);
      expect(strip()).toHaveTextContent(E('cliAgentEgress'));

      stub.provider = 'ollama';
      rerender(<GenerationOutput {...makeProps({ activeOut: 'resume', resumeId: 'resume-1' })} />);
      expect(strip()).not.toHaveTextContent(E('cliAgentEgress'));
    });

    it('does NOT disclose egress on the no-résumé reason — nothing was ever sent', () => {
      stub.provider = 'claude-code';
      renderStrip({ resumeId: undefined });
      expect(strip()).not.toHaveTextContent(E('cliAgentEgress'));
    });

    // ── Finding 1 regression: the snapshot must survive the strip unmounting ──
    // The strip only renders on `view === 'doc' && activeOut === 'resume'`, so
    // switching to the Job ad tab unmounts it. The snapshot MUST be owned by
    // GenerationOutput (which stays mounted) — a snapshot re-initialised on
    // the strip's own remount would silently score the EDITED posting text,
    // through a path that can route via translation.

    it('a tab switch away and back still scores the ORIGINAL snapshot, not a jobDesc edited while away', async () => {
      const user = userEvent.setup();
      setScore();
      const props = makeProps({
        activeOut: 'resume',
        resumeId: 'resume-1',
        jobDesc: 'Original posting text',
      });
      const { rerender } = render(<GenerationOutput {...props} />);

      // Switch to the Job ad tab — the strip unmounts.
      await clickJobAdTab(user);
      expect(stripQuery()).not.toBeInTheDocument();

      // Simulate the posting being edited on the Job ad sub-tab while the
      // strip is unmounted — GenerationOutput is controlled, so the parent
      // would pass this back down as a new `jobDesc`.
      rerender(<GenerationOutput {...props} jobDesc="Edited posting text" />);

      // Switch back to the résumé tab — the strip remounts.
      await user.click(screen.getByRole('tab', { name: 'autopilot.apply.target.resume' }));

      const enabledCalls = mockUseJobAdTextMatchScore.mock.calls.filter((call) => call[2] === true);
      const lastEnabledCall = enabledCalls[enabledCalls.length - 1];
      expect(lastEnabledCall?.[1]).toBe('Original posting text');
    });
  });
});
