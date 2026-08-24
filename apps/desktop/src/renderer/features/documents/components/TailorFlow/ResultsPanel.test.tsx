import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { PipelineRunSummary } from '@ajh/shared/ipc';
import type * as AjhTranslations from '@ajh/translations';

import { ResultsPanel } from './ResultsPanel';

// Echo every key verbatim — no i18next runtime needed in jsdom. Only
// `useTranslation` is replaced; every other export stays real, so adding one
// doesn't silently break this file.
vi.mock('@ajh/translations', async (importOriginal) => ({
  ...(await importOriginal<typeof AjhTranslations>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

// The viewer itself is covered by GenerationOutput.test.tsx; here it is a marker
// so the assertions are about ResultsPanel's own layout shape only.
vi.mock('./GenerationOutput', () => ({
  GenerationOutput: () => <div data-testid="generation-output-stub" />,
}));

// The list itself is covered by PipelineRunsList.test.tsx; here it is a marker
// so the assertions are about ResultsPanel's OWN modal-gating (M1), not the
// list's row rendering.
vi.mock('@/components/generation/PipelineRunsList', () => ({
  PipelineRunsList: ({ runs }: { runs: PipelineRunSummary[] }) => (
    <div data-testid="pipeline-runs-list-stub">{runs.length} runs</div>
  ),
}));

function run(overrides: Partial<PipelineRunSummary> = {}): PipelineRunSummary {
  return {
    runId: 'run-1',
    jobUrl: 'https://example.test/job',
    kind: 'resume',
    depth: 'quality',
    status: 'completed',
    startedAt: Date.now() - 60_000,
    finishedAt: Date.now(),
    stoppedReason: 'done',
    metrics: { calls: 4, repairRounds: 1 },
    ...overrides,
  };
}

function makeProps(): ComponentProps<typeof ResultsPanel> {
  return {
    target: 'both',
    hasResume: true,
    jobDesc: 'Full job description text',
    onJobDescChange: vi.fn(),
    hasDesc: true,
    fetchingDesc: false,
    jobUrl: 'https://example.com/job',
    jobAdSummary: {
      summary: '',
      generating: false,
      error: null,
      generate: vi.fn(),
      language: 'en',
      setLanguage: vi.fn(),
    },
    activeOut: 'resume',
    setActiveOut: vi.fn(),
    templateId: 'classic',
    atsMode: false,
    accent: undefined,
    letterLayoutId: undefined,
    onTemplateChange: vi.fn(),
    onAtsModeChange: vi.fn(),
    onAccentChange: vi.fn(),
    onLetterLayoutChange: vi.fn(),
    output: 'Generated resume content',
    onEdit: vi.fn(),
    meta: null,
    copied: false,
    onCopy: vi.fn(),
    exportOpen: false,
    setExportOpen: vi.fn(),
    onExport: vi.fn(),
    runState: 'done',
    onRegenerate: vi.fn(),
    onEditSettings: vi.fn(),
  };
}

// Regression guard for the "document viewer header scrolls away" bug: this panel
// used to wrap GenerationOutput in an `overflow-y-auto` body, so IT scrolled the
// whole viewer — pinned header included. The scroll boundary belongs inside
// GenerationOutput (on its tabpanel); this body must stay height-bounded only.
describe('ResultsPanel layout', () => {
  const SCROLLS = /overflow-(?:y-)?(?:auto|scroll)/;

  it('does not scroll the viewer — no scroll container above GenerationOutput', () => {
    const { container } = render(<ResultsPanel {...makeProps()} />);

    for (
      let el = screen.getByTestId('generation-output-stub').parentElement;
      el !== null && el !== container;
      el = el.parentElement
    ) {
      expect(el.className).not.toMatch(SCROLLS);
    }
  });

  it('bounds the viewer body so it can never grow past the panel', () => {
    render(<ResultsPanel {...makeProps()} />);

    const body = screen.getByTestId('generation-output-stub').parentElement;
    expect(body).not.toBeNull();
    expect(body?.className).toContain('min-h-0');
    expect(body?.className).toContain('flex-1');
  });

  it('keeps the regenerate / edit-settings footer pinned below the body', () => {
    render(<ResultsPanel {...makeProps()} />);

    const footer = screen
      .getByRole('button', { name: 'autopilot.apply.wizard.results.regenerate' })
      .closest('div');
    expect(footer).not.toBeNull();
    expect(footer?.className).toContain('shrink-0');
    expect(footer?.contains(screen.getByTestId('generation-output-stub'))).toBe(false);
  });

  it('forwards the footer actions to their callbacks', () => {
    const props = makeProps();
    render(<ResultsPanel {...props} />);

    screen.getByRole('button', { name: 'autopilot.apply.wizard.results.regenerate' }).click();
    screen.getByRole('button', { name: 'autopilot.apply.wizard.results.edit' }).click();

    expect(props.onRegenerate).toHaveBeenCalledTimes(1);
    expect(props.onEditSettings).toHaveBeenCalledTimes(1);
  });
});

describe('ResultsPanel — status banner', () => {
  it('shows nothing extra on a clean finish (runState=done)', () => {
    render(<ResultsPanel {...makeProps()} runState="done" />);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows the needsReview banner with the unresolved claim count', () => {
    render(
      <ResultsPanel
        {...makeProps()}
        runState="needsReview"
        openClaims={2}
        pipelineReview={{
          documentText: 'Built the pipeline with 250 users impact.',
          sections: [],
          fabrications: [
            { issueKey: 'a#0', code: 'a', evidence: '250 users' },
            { issueKey: 'a#1', code: 'a', evidence: 'never in the document' },
          ],
        }}
      />
    );
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent('autopilot.apply.wizard.results.needsReviewTitle');
    expect(status).not.toHaveTextContent('autopilot.apply.wizard.results.needsReviewTitleEmpty');
    expect(status).toHaveTextContent('autopilot.apply.wizard.results.needsReviewHint');
  });

  // H5: report.rs can flag `needsReview` off a critical with no fabrication
  // entry (`slot_has_unresolvable_critical`) — "0 claims need your verdict"
  // then points at an empty list. Branch to honest copy instead.
  it('shows the zero-claims variant when needsReview carries no open claims', () => {
    render(<ResultsPanel {...makeProps()} runState="needsReview" openClaims={0} />);
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent('autopilot.apply.wizard.results.needsReviewTitleEmpty');
    expect(status).toHaveTextContent('autopilot.apply.wizard.results.needsReviewHintEmpty');
    // CR-4: `needsReviewHint` is a text PREFIX of `needsReviewHintEmpty` (which
    // DOES render above) — a plain substring `not.toHaveTextContent` would
    // false-negative on that prefix match. The negative lookahead excludes
    // exactly the `Empty`-suffixed occurrence, asserting the BARE
    // non-empty-claims hint specifically is absent.
    expect(status).not.toHaveTextContent(/needsReviewHint(?!Empty)/);
  });

  // H5 (cold half): a COLD redisplay has no interactive fix/resolve UI wired
  // (no live runId) — the hint must say the run needs reopening, not imply an
  // action available right here.
  it('shows the cold-entry needsReview hint when cold=true', () => {
    render(<ResultsPanel {...makeProps()} runState="needsReview" openClaims={1} cold />);
    expect(screen.getByRole('status')).toHaveTextContent(
      'autopilot.apply.wizard.results.needsReviewHintCold'
    );
  });

  // H4: a run can report `status: completed` (→ runState='done') while still
  // carrying `stoppedReason: 'run_timeout'` (hooks.rs's
  // `timed_out_with_document`) — the banner must not be suppressed just
  // because `runState === 'done'`.
  it('shows the stopped-reason banner even when runState is "done" (timed-out-but-saved run)', () => {
    render(<ResultsPanel {...makeProps()} runState="done" stoppedReason="run_timeout" />);
    expect(screen.getByText('pipeline.stopped.runTimeout')).toBeInTheDocument();
  });

  it('does NOT show a banner for a genuinely clean done run (stoppedReason absent)', () => {
    render(<ResultsPanel {...makeProps()} runState="done" stoppedReason={null} />);
    expect(screen.queryByText(/pipeline\.stopped\./)).not.toBeInTheDocument();
  });

  it('does NOT show a banner for a done run whose stoppedReason is literally "done"', () => {
    render(<ResultsPanel {...makeProps()} runState="done" stoppedReason="done" />);
    expect(screen.queryByText(/pipeline\.stopped\./)).not.toBeInTheDocument();
  });

  it('shows the failed banner with the error detail text', () => {
    render(<ResultsPanel {...makeProps()} runState="error" error="Model timed out" />);
    const alert = screen.getByRole('alert');
    expect(alert).toHaveTextContent('autopilot.apply.wizard.results.failedTitle');
    expect(alert).toHaveTextContent('Model timed out');
  });

  it('shows the cancelled hint without an alert/status role', () => {
    render(<ResultsPanel {...makeProps()} runState="cancelled" />);
    expect(screen.getByText('autopilot.apply.wizard.results.cancelledHint')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows the stopped-reason suffix when the run carries one', () => {
    render(<ResultsPanel {...makeProps()} runState="cancelled" stoppedReason="cancelled" />);
    expect(screen.getByText('pipeline.stopped.cancelled')).toBeInTheDocument();
  });
});

describe('ResultsPanel — run history (M1: behind a click, not permanently mounted)', () => {
  it('renders nothing extra when there is no run history', () => {
    render(<ResultsPanel {...makeProps()} runs={[]} />);
    expect(screen.queryByText('pipeline.runs.title')).not.toBeInTheDocument();
    expect(screen.queryByTestId('pipeline-runs-list-stub')).not.toBeInTheDocument();
  });

  it('shows a trigger button, not the list itself, when run history exists', () => {
    render(<ResultsPanel {...makeProps()} runs={[run()]} />);
    expect(screen.getByRole('button', { name: 'pipeline.runs.title' })).toBeInTheDocument();
    // The list is portalled by ModalShell but stays closed (empty) until clicked.
    expect(screen.queryByText('1 runs')).not.toBeInTheDocument();
  });

  it('opens the run-history modal on click, and closes it again', async () => {
    const user = userEvent.setup();
    render(<ResultsPanel {...makeProps()} runs={[run(), run({ runId: 'run-0' })]} />);

    await user.click(screen.getByRole('button', { name: 'pipeline.runs.title' }));
    expect(screen.getByTestId('pipeline-runs-list-stub')).toHaveTextContent('2 runs');

    await user.click(screen.getByRole('button', { name: 'common.close' }));
    expect(screen.queryByTestId('pipeline-runs-list-stub')).not.toBeInTheDocument();
  });
});

describe('ResultsPanel — "all steps completed" summary (H2)', () => {
  it('shows a 4-check summary on a clean finish', () => {
    render(<ResultsPanel {...makeProps()} runState="done" stoppedReason={null} />);
    for (const key of ['analyze', 'generate', 'validate', 'humanize']) {
      expect(screen.getByText(`pipeline.step.${key}.label`)).toBeInTheDocument();
    }
  });

  it('does NOT show the summary for a truncated-but-"done" run (H4 case)', () => {
    render(<ResultsPanel {...makeProps()} runState="done" stoppedReason="run_timeout" />);
    expect(screen.queryByText('pipeline.step.analyze.label')).not.toBeInTheDocument();
  });

  it('does NOT show the summary for needsReview/cancelled/error', () => {
    for (const runState of ['needsReview', 'cancelled', 'error'] as const) {
      const { unmount } = render(<ResultsPanel {...makeProps()} runState={runState} />);
      expect(screen.queryByText('pipeline.step.analyze.label')).not.toBeInTheDocument();
      unmount();
    }
  });

  // N3: was text-foreground/45 (4.22:1 dark, below the AA text floor) — the
  // repo's documented sub-14px floor is /70 (8.04:1 dark / 6.20:1 light).
  it('renders the summary row at the AA-safe text-foreground/70, not /45', () => {
    render(<ResultsPanel {...makeProps()} runState="done" stoppedReason={null} />);
    const row = screen.getByText('pipeline.step.analyze.label').closest('div');
    expect(row).not.toBeNull();
    expect(row?.className).toContain('text-foreground/70');
    expect(row?.className).not.toContain('text-foreground/45');
  });
});
