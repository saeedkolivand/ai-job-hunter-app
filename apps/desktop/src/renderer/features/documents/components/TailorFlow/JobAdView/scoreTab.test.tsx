/**
 * JobAdView — Score tab structure (raw-key i18n echo): presence, empty-state
 * wiring, and the query-gating guard against a per-keystroke scoring storm.
 *
 * The "never a 0" / "never reads ATS score" guarantees are covered against REAL
 * translated copy in the `*.i18n.test.tsx` files (this file's `t` is a raw-key
 * echo, which can't catch either regression).
 */
import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { JobAdView } from '../JobAdView';
import { makeProps, mockUseJobAdTextMatchScore } from './test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/components/ui/ModelSelector', async () => {
  return (await import('./test-support')).modelSelectorModule;
});
vi.mock('@/components/ui/ExternalLink', async () => {
  return (await import('./test-support')).externalLinkModule;
});
vi.mock('@/lib/generate', async () => (await import('./test-support')).generateModule);
vi.mock('@/services', async () => (await import('./test-support')).servicesModule);

// ── Score tab — presence + empty-state wiring ────────────────────────────────

describe('JobAdView — Score tab presence and empty states', () => {
  it('renders a third "score" tab option alongside summary/source', () => {
    render(<JobAdView {...makeProps()} />);
    expect(screen.getByText('autopilot.apply.jobAdView.scoreTab')).toBeInTheDocument();
  });

  it('shows the no-resume reason when resumeId is absent (never a score)', async () => {
    render(<JobAdView {...makeProps({ resumeId: undefined })} />);
    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.scoreTab'));
    expect(screen.getByText('jobs.scoreNoResume')).toBeInTheDocument();
  });

  it('shows the no-posting reason when resumeId is present but the snapshot is empty', async () => {
    render(<JobAdView {...makeProps({ resumeId: 'resume-1', jobDesc: '' })} />);
    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.scoreTab'));
    expect(screen.getByText('autopilot.apply.jobAdView.score.noPosting')).toBeInTheDocument();
  });

  it('treats a whitespace-only posting as empty (no-posting reason, not a score)', async () => {
    render(<JobAdView {...makeProps({ resumeId: 'resume-1', jobDesc: '   ' })} />);
    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.scoreTab'));
    expect(screen.getByText('autopilot.apply.jobAdView.score.noPosting')).toBeInTheDocument();
  });
});

// ── 10. Score tab query gating — no per-keystroke storm ──────────────────────
// The hazard: `useJobAdTextMatchScore`'s query key is content-addressed on the
// job text, and the Job Ad sub-tab right next to Score is a live-editing
// textarea. A naive wire-up (pass `jobDesc` straight through, gate `enabled`
// only on `tab === 'score'`) still re-enables the query with a NEW key on
// every keystroke typed while the tab happens to stay open. The fix
// snapshots `jobDesc` once, at the moment the tab opens. Asserted against the
// mocked hook's actual `enabled` argument (absolute counts), not a
// before/after comparison — see JobAdView's `handleTabChange`.

/** Controlled wrapper — `makeProps()`'s `onJobDescChange` is a no-op `vi.fn`,
 *  so typing in the real component needs a real state loop backing it. */
function ControlledJobAdView(overrides: Partial<Parameters<typeof JobAdView>[0]> = {}) {
  const [jobDesc, setJobDesc] = React.useState(overrides.jobDesc ?? '');
  return <JobAdView {...makeProps({ ...overrides, jobDesc, onJobDescChange: setJobDesc })} />;
}

describe('JobAdView — Score tab query gating (no per-keystroke storm)', () => {
  beforeEach(() => {
    mockUseJobAdTextMatchScore.mockClear();
  });

  // The real property under test is DISTINCT query keys fired, not raw call
  // count — the mock is invoked on every render (Rules of Hooks) regardless
  // of `enabled`, so an incidental extra re-render with the SAME (already-
  // enabled) key would fail a plain length check while behaviour never
  // actually changed. Counting `enabled` calls by their distinct `jobText`
  // argument survives that.
  function distinctEnabledKeyCount() {
    return new Set(
      mockUseJobAdTextMatchScore.mock.calls
        .filter((call) => call[2] === true)
        .map((call) => call[1])
    ).size;
  }

  it('typing on the source tab never enables the query; opening Score enables it exactly once', async () => {
    render(
      <ControlledJobAdView
        resumeId="resume-1"
        jobDesc="Full description that looks truncated..."
        hasDesc
      />
    );
    // Truncated ("...") → starts on the source tab, textarea already visible.
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);

    await userEvent.type(textarea, 'more');
    expect(distinctEnabledKeyCount()).toBe(0);

    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.scoreTab'));
    expect(distinctEnabledKeyCount()).toBe(1);
  });

  it('re-opening the Score tab re-scores, but editing while it stays open does not', async () => {
    render(
      <ControlledJobAdView
        resumeId="resume-1"
        jobDesc="Full description that looks truncated..."
        hasDesc
      />
    );

    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.scoreTab'));
    expect(distinctEnabledKeyCount()).toBe(1);

    // Back to source, edit, and stay there — the (now closed) Score tab's
    // query must not flip enabled again from a live jobDesc change. The
    // textarea is RE-QUERIED here (not the reference from before the tab
    // switch) — the source tab's whole subtree, textarea included, unmounts
    // while the Score tab is showing, so a stale node would silently no-op.
    await userEvent.click(screen.getByText('autopilot.apply.tabs.jobAd'));
    const textarea = screen.getByTestId(TEST_IDS.documents.jobAdViewTextarea);
    await userEvent.type(textarea, ' plus some edits');
    expect(distinctEnabledKeyCount()).toBe(1);

    // Re-opening IS the explicit action that re-scores the edited (DIFFERENT)
    // text — a genuinely new key, not just another render.
    await userEvent.click(screen.getByText('autopilot.apply.jobAdView.scoreTab'));
    expect(distinctEnabledKeyCount()).toBe(2);
  });
});
