/**
 * useTailorPipeline — session-derived state: checklist step, runStartedAt, run notification, stage label, report fallback, invalidation.
 * Mocks + render helpers live in `harness.ts` (see its header).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import type { AiGenerationRecord } from '@ajh/shared';

import { keys } from '@/services/query-client';

import {
  detail,
  getQueryClient,
  PARAMS,
  render,
  resetHarness,
  sessionBus,
  useTailorPipelineForTest,
  wrapper,
} from './harness';

beforeEach(resetHarness);

describe('useTailorPipeline — the 4-step checklist position', () => {
  it('advances currentStep as the stage moves through the pipeline, never regressing on an unknown stage', () => {
    const { result, rerender } = render();
    expect(result.current.currentStep).toBe(0);

    sessionBus.stage = { stage: 'draft', phase: 'start' };
    rerender();
    expect(result.current.currentStep).toBe(1);

    sessionBus.stage = { stage: 'validate', phase: 'start' };
    rerender();
    expect(result.current.currentStep).toBe(2);

    // A stage name this build doesn't map — it holds rather than regressing.
    sessionBus.stage = { stage: 'a_future_stage', phase: 'start' };
    rerender();
    expect(result.current.currentStep).toBe(2);
  });
});

// The elapsed-timer fix (owner report: the clock reset to 0:00 on
// navigate-away-and-back): `GeneratingPanel` anchors on this field instead
// of its own mount time, so it must mirror the run RECORD's own backend
// timestamp — not anything reset by a remount of this hook.
describe('useTailorPipeline — runStartedAt (backend-anchored elapsed timer)', () => {
  it.each([
    ['is null before any run record has loaded', undefined, null],
    ["mirrors the run record's own startedAt once it loads", 12_345, 12_345],
    // Defensive: not reachable with today's `now_ms()`-populated
    // `pipeline_runs.started_at` column, but `?? null` alone only guards
    // null/undefined — a `0` (or negative) value would otherwise become the
    // anchor and render an absurd/negative "N total" caption. `null` here is
    // what lets `GeneratingPanel`'s own `runStartedAt ?? mountFallback` recover
    // instead.
    ['falls back to null (not 0) for a non-positive startedAt', 0, null],
  ])('%s', (_name, startedAt, expected) => {
    if (startedAt !== undefined) {
      sessionBus.detail = detail({ status: 'running', startedAt });
    }
    const { result } = render();
    expect(result.current.runStartedAt).toBe(expected);
  });

  // The actual owner-reported path: navigating away unmounts the whole flow
  // (`ApplicationDetailPage` only renders `TailorFlow` while the Documents tab
  // is active) and a fresh mount reconnects via `initialRunId`/`initialJobId`.
  // A brand-new hook instance must still read the ORIGINAL start time off the
  // reconnected run record, not restart it.
  it('survives an unmount/remount ("navigate away and back") unchanged', () => {
    sessionBus.detail = detail({ status: 'running', startedAt: 12_345 });
    const first = render();
    expect(first.result.current.runStartedAt).toBe(12_345);
    first.unmount();

    const second = render({ initialRunId: 'run-1', initialJobId: 'job-1' });
    expect(second.result.current.runStartedAt).toBe(12_345);
  });
});

describe('useTailorPipeline — persisted-run notification', () => {
  it.each([
    ['calls onRunStarted once both ids are known', 'job-1', true],
    ['never calls onRunStarted while only one id is known', null, false],
  ])('%s', (_name, jobId, called) => {
    const onRunStarted = vi.fn();
    sessionBus.runId = 'run-1';
    sessionBus.jobId = jobId;
    render({ onRunStarted });
    if (called) expect(onRunStarted).toHaveBeenCalledWith({ runId: 'run-1', jobId: 'job-1' });
    else expect(onRunStarted).not.toHaveBeenCalled();
  });

  // F1 regression: on the real DocumentsTab/TailorFlow wiring, `onRunStarted`
  // writes a Zustand slice, which ALWAYS returns a new object — re-rendering
  // the host, which passes a brand-new arrow back in. The prior effect listed
  // `onRunStarted` as a dependency with no already-persisted guard, so this
  // reproduced "Maximum update depth exceeded" immediately after a run
  // started. A test with a stable `vi.fn()` (the two tests above) cannot
  // catch this — it must pass a FRESH arrow every render, exactly like the
  // real host does.
  it('does not loop when onRunStarted is a fresh arrow every render (F1)', () => {
    const persisted: { runId: string; jobId: string }[] = [];
    sessionBus.runId = 'run-1';
    sessionBus.jobId = 'job-1';

    const { rerender } = renderHook(
      (props: { onRunStarted: (ids: { runId: string; jobId: string }) => void }) =>
        useTailorPipelineForTest({ ...PARAMS, ...props }),
      {
        wrapper,
        initialProps: { onRunStarted: (ids) => persisted.push(ids) },
      }
    );

    // 20 re-renders, each passing a NEW closure — the exact shape that broke
    // (TailorFlow's inline `onRunStarted: (ids) => { persistence.setRun(...) }`).
    // If the guard regresses, this either throws React's max-update-depth
    // error or the callback fires 20 times instead of once.
    for (let i = 0; i < 20; i++) {
      rerender({ onRunStarted: (ids) => persisted.push(ids) });
    }

    expect(persisted).toEqual([{ runId: 'run-1', jobId: 'job-1' }]);
  });

  it('persists again once a NEW run id replaces the old one (guard is keyed, not one-shot)', () => {
    const onRunStarted = vi.fn();
    sessionBus.runId = 'run-1';
    sessionBus.jobId = 'job-1';
    const { rerender } = render({ onRunStarted });
    expect(onRunStarted).toHaveBeenCalledTimes(1);

    sessionBus.runId = 'run-2';
    sessionBus.jobId = 'job-2';
    rerender();

    expect(onRunStarted).toHaveBeenCalledTimes(2);
    expect(onRunStarted).toHaveBeenLastCalledWith({ runId: 'run-2', jobId: 'job-2' });
  });
});

describe('useTailorPipeline — stageLabel fallback (L: no raw snake_case leak)', () => {
  it('falls back to the translated coarse state for a stage name this build does not have copy for', () => {
    sessionBus.state = 'drafting';
    sessionBus.stage = {
      stage: 'a_future_stage_this_build_predates',
      phase: 'start',
    };
    const { result } = render();
    // Never the raw wire name — falls back to pipeline.state.drafting's translation.
    expect(result.current.stageLabel).not.toBe('a_future_stage_this_build_predates');
    expect(result.current.stageLabel).toBe('pipeline.state.drafting');
  });
});

describe('useTailorPipeline — quality report survives a terminal run with no live report', () => {
  const minimalReport = {
    ok: true,
    issues: [],
    metrics: {
      keywordCoverage: null,
      topRequirementHits: 3,
      duplicateRatio: 0,
      rolesSource: 0,
      rolesOutput: 0,
    },
  };

  /** An aggregate record whose persisted `qualityReport` wraps `report` for the résumé slot. */
  const withReport = (report: typeof minimalReport, sourceTextHash: number) =>
    ({
      id: 'gen-1',
      coverLetterText: '',
      qualityReport: JSON.stringify({
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: { report, sourceTextHash },
      }),
    }) as AiGenerationRecord;

  it('falls back to the persisted report when a terminal detail.report is null', () => {
    sessionBus.detail = detail({ status: 'cancelled', report: null });
    const { result } = render({ latestGeneration: withReport(minimalReport, 0) });

    expect(result.current.report).not.toBeNull();
    expect(result.current.report?.resume?.report.metrics.topRequirementHits).toBe(3);
  });

  it('still prefers the live report when both exist', () => {
    sessionBus.detail = detail({
      status: 'completed',
      report: {
        schemaVersion: 2,
        pipeline: 'quality',
        generatedAt: 0,
        resume: { report: { ...minimalReport, ok: false }, sourceTextHash: 1 },
      },
    });
    const { result } = render({ latestGeneration: withReport(minimalReport, 0) });

    expect(result.current.report?.resume?.sourceTextHash).toBe(1);
  });
});

// Stage 6e — a hard failure via the umbrella `job.failed` path (a full queue,
// no configured provider, a deleted résumé, …) sends `ERROR` straight to the
// session machine with NO run record ever written (`detail` stays `null`).
// The old effect gated on `session.detail?.status`, which never exists here,
// so the aggregate (and Autopilot's score) never refreshed. Reverting to that
// gate makes this fail.
describe('useTailorPipeline — a hard failure with no run record still invalidates', () => {
  it('invalidates aiGenerations AND autopilot once the session reaches ERROR with detail still null', () => {
    const invalidateSpy = vi.spyOn(getQueryClient(), 'invalidateQueries');
    const { rerender } = render();
    invalidateSpy.mockClear();

    sessionBus.state = 'error';
    sessionBus.detail = null;
    rerender();

    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: keys.aiGenerations.all });
    expect(invalidateSpy).toHaveBeenCalledWith({ queryKey: keys.autopilot.all });
  });

  it('does not invalidate while still busy or idle', () => {
    const invalidateSpy = vi.spyOn(getQueryClient(), 'invalidateQueries');
    render();
    expect(invalidateSpy).not.toHaveBeenCalled();

    sessionBus.state = 'drafting';
    const { rerender } = render();
    rerender();
    expect(invalidateSpy).not.toHaveBeenCalled();
  });
  // ── which document the panel opens on ──────────────────────────────────────
  //
  // The user-visible half of the reported bug: `activeOut` used to be a plain
  // `useState('resume')` that nothing ever corrected from the run's target, and
  // `GenerationOutput` builds its TAB LIST from it — so a cover-only run
  // rendered exactly one tab, labelled "Resume", showing the résumé.
});
