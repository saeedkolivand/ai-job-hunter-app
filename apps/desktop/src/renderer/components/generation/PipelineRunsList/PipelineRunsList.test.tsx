import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { PipelineRunEvent, PipelineRunSummary } from '@ajh/shared/ipc';

import { PipelineRunsList } from './PipelineRunsList';

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

describe('PipelineRunsList', () => {
  it('says so when a posting has no staged runs', () => {
    render(<PipelineRunsList runs={[]} />);
    expect(screen.getByText(/no staged runs yet/i)).toBeInTheDocument();
  });

  it('shows status, depth and metrics per run', () => {
    render(<PipelineRunsList runs={[run()]} />);
    expect(screen.getByText('Completed')).toBeInTheDocument();
    expect(screen.getByText('Quality')).toBeInTheDocument();
    expect(screen.getByText(/4 calls · 1 repair rounds/)).toBeInTheDocument();
  });

  it('opens a run report', async () => {
    const onOpenReport = vi.fn();
    render(<PipelineRunsList runs={[run()]} onOpenReport={onOpenReport} />);
    await userEvent.click(screen.getByRole('button', { name: /open report/i }));
    expect(onOpenReport).toHaveBeenCalledWith('run-1');
  });

  it('renders needsReview as its own state — not a success, not a failure', () => {
    render(<PipelineRunsList runs={[run({ status: 'needsReview', stoppedReason: 'done' })]} />);
    expect(screen.getByText('Needs review')).toBeInTheDocument();
    expect(screen.queryByText('Completed')).not.toBeInTheDocument();
  });

  // `stoppedReason` is nullable BY CONTRACT on a terminal run: a failure with no
  // recorded reason carries none, and `status` is the authority. Falling back to
  // the `done` label here would report a failed run as finished — exactly what
  // the backend's `terminal_state` was fixed to stop doing.
  describe('a nullable stoppedReason', () => {
    it.each([
      ['null', null],
      ['undefined', undefined],
      ['an empty string', ''],
    ])('renders no reason at all for %s, never "Finished"', (_label, stoppedReason) => {
      render(<PipelineRunsList runs={[run({ status: 'failed', stoppedReason })]} />);
      expect(screen.getByText('Failed')).toBeInTheDocument();
      expect(screen.queryByText(/Finished/)).not.toBeInTheDocument();
    });

    it('labels a reason this build does not know vaguely rather than wrongly', () => {
      render(
        <PipelineRunsList runs={[run({ status: 'failed', stoppedReason: 'quantum_flux' })]} />
      );
      expect(screen.getByText(/Stopped/)).toBeInTheDocument();
      expect(screen.queryByText(/Finished/)).not.toBeInTheDocument();
    });

    it('does use the mapped label when the backend gave one', () => {
      render(<PipelineRunsList runs={[run({ status: 'failed', stoppedReason: 'run_timeout' })]} />);
      expect(screen.getByText(/ran out of time/i)).toBeInTheDocument();
    });
  });

  describe('stage timeline', () => {
    const events: PipelineRunEvent[] = [
      { seq: 0, ts: 1, stage: 'analyze_job', phase: 'start', artifact: {} },
      { seq: 1, ts: 2, stage: 'analyze_job', phase: 'finish', artifact: {} },
    ];

    it('renders the persisted trail when the caller has it', async () => {
      render(<PipelineRunsList runs={[run()]} eventsByRun={{ 'run-1': events }} />);
      await userEvent.click(screen.getByRole('button', { name: /show stage timeline/i }));
      expect(screen.getAllByText('Reading the job ad')).toHaveLength(2);
    });

    // `phase` is a Rust enum name. Rendering it raw put `start`/`finish` — English
    // identifiers — in front of a German user.
    it('translates the phase instead of printing the raw enum name', async () => {
      const { container } = render(
        <PipelineRunsList runs={[run()]} eventsByRun={{ 'run-1': events }} />
      );
      await userEvent.click(screen.getByRole('button', { name: /show stage timeline/i }));
      const trail = container.querySelector('ol')?.textContent ?? '';
      expect(trail).toContain('started');
      expect(trail).toContain('finished');
    });

    it('pairs aria-controls with the region it actually renders', async () => {
      render(<PipelineRunsList runs={[run()]} eventsByRun={{ 'run-1': events }} />);
      const toggle = screen.getByRole('button', { name: /show stage timeline/i });
      // Collapsed: nothing to point AT, so no dangling reference.
      expect(toggle).not.toHaveAttribute('aria-controls');

      await userEvent.click(toggle);
      const id = screen
        .getByRole('button', { name: /hide stage timeline/i })
        .getAttribute('aria-controls');
      expect(id).toBeTruthy();
      expect(document.getElementById(id ?? '')).toBeInTheDocument();
    });

    // The list endpoint returns SUMMARIES, so a trail the caller never fetched
    // genuinely isn't known — say that instead of drawing an empty ladder that
    // reads as "this run did nothing".
    it('says the trail is unavailable rather than faking one', async () => {
      const onExpand = vi.fn();
      render(<PipelineRunsList runs={[run()]} onExpand={onExpand} />);
      await userEvent.click(screen.getByRole('button', { name: /show stage timeline/i }));
      expect(screen.getByText(/live progress only/i)).toBeInTheDocument();
      expect(onExpand).toHaveBeenCalledWith('run-1');
    });

    // A max-depth run's PERSISTED trail still carries `sections`/`assemble`/
    // `llm_judge` — the max pipeline that emitted them is gone, but rows it
    // already wrote are not, and nothing migrates them. `defaultValue` keeps a
    // missing key from rendering as a dotted string, but a raw stage token
    // ("sections", "assemble") is still an English wire identifier landing in
    // front of, e.g., a German user — the exact failure `translates the phase
    // instead of printing the raw enum name` above guards for `phase`. This is
    // the same guard for `stage`, and it has to be a RENDER assertion against
    // persisted events, not a key-existence check: a test that only asserts
    // `pipeline.stage.sections` exists in the JSON would pass even if this
    // component stopped reading it. Mutation-tested: deleting any one of the
    // three restored locale keys reddens the matching row's assertion below.
    it('renders human labels for the retired max-only stages in a persisted trail', async () => {
      const maxEvents: PipelineRunEvent[] = [
        { seq: 0, ts: 1, stage: 'sections', phase: 'start', artifact: {} },
        { seq: 1, ts: 2, stage: 'sections', phase: 'finish', artifact: {} },
        { seq: 2, ts: 3, stage: 'assemble', phase: 'finish', artifact: {} },
        { seq: 3, ts: 4, stage: 'llm_judge', phase: 'finish', artifact: {} },
      ];
      render(<PipelineRunsList runs={[run()]} eventsByRun={{ 'run-1': maxEvents }} />);
      await userEvent.click(screen.getByRole('button', { name: /show stage timeline/i }));

      // Each stage's own <span> holds nothing but its translated label, so a
      // plain-string query (whole-node match, not a substring) is exact — no
      // risk of "assemble"'s translation ("Putting the sections together")
      // colliding with a raw "sections" token check the way a substring/regex
      // scan over the whole trail's text would.
      expect(screen.getAllByText('Writing the résumé section by section')).toHaveLength(2);
      expect(screen.getByText('Putting the sections together')).toBeInTheDocument();
      expect(screen.getByText('Reading the finished résumé once more')).toBeInTheDocument();
      // Not the raw wire tokens — the exact regression this guard exists for.
      expect(screen.queryByText('sections')).not.toBeInTheDocument();
      expect(screen.queryByText('assemble')).not.toBeInTheDocument();
      expect(screen.queryByText('llm_judge')).not.toBeInTheDocument();
    });
  });

  it('warns that only the newest run still has its document', () => {
    render(<PipelineRunsList runs={[run(), run({ runId: 'run-0', startedAt: 1 })]} />);
    expect(screen.getByText(/only the newest run's document is stored/i)).toBeInTheDocument();
  });

  // PR-4 deleted the depth SELECTORS once the apply flow stopped offering a
  // choice, but a user's run history still has rows at all three depths — this
  // is the one surface left that renders that history, so the label vocabulary
  // (`generationDepth.option.*.label`) has to survive the cleanup even though
  // nothing writes those depths anymore. `max` in particular: it is the depth
  // most likely to look "orphaned" to a future pass, since nothing can select
  // it and no other test in this repo renders it.
  describe('historic depth labels', () => {
    it.each(
      // en labels, asserted against the real locale JSON — see the other tests
      // in this file for why a human label (not the raw token) is the bar.
      [
        ['fast', 'Fast'],
        ['quality', 'Quality'],
        ['max', 'Max'],
      ] as const
    )('renders a human label for a historic "%s" run, not the raw token', (depth, label) => {
      render(<PipelineRunsList runs={[run({ depth })]} />);
      expect(screen.getByText(label)).toBeInTheDocument();
      expect(screen.queryByText(`generationDepth.option.${depth}.label`)).not.toBeInTheDocument();
    });
  });
});
