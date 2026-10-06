// useGeneration — quality report wiring, stale-persist guard, supersede race.
import { describe, expect, it, vi } from 'vitest';

import { computeQualityReport, generateCoverLetter, generateResume } from '@/lib/generate';

import {
  installGenerationMocks,
  runGeneration,
  setup,
  stageCalls,
} from './useGeneration.test-support';

vi.mock('@/lib/generate', async () => (await import('./useGeneration.stubs')).generateMock());

installGenerationMocks();

describe('useGeneration — quality report wiring', () => {
  it('stores the report and carries it on the save when validation succeeds', async () => {
    const report = {
      schemaVersion: 2 as const,
      pipeline: 'fast' as const,
      generatedAt: 1,
      resume: {
        report: {
          ok: true,
          issues: [],
          metrics: {
            keywordCoverage: null,
            topRequirementHits: 0,
            duplicateRatio: 0,
            rolesSource: 0,
            rolesOutput: 0,
          },
        },
        sourceTextHash: 1,
      },
    };
    vi.mocked(computeQualityReport).mockResolvedValueOnce(report);
    const { m } = await runGeneration('resume');

    expect(m.setReport).toHaveBeenCalledWith(report);
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledWith(
      expect.objectContaining({ qualityReport: JSON.stringify(report) })
    );
  });

  it('degrades to a report-less save when validation returns null, never blocking the save', async () => {
    vi.mocked(computeQualityReport).mockResolvedValueOnce(null);
    const { m } = await runGeneration('resume');

    expect(m.setReport).toHaveBeenCalledWith(null);
    expect(m.saveAiGeneration.mutate).toHaveBeenCalled();
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalledWith(
      expect.objectContaining({ qualityReport: expect.anything() })
    );
  });

  it('clears the previous report before the progressive-reveal window, so a regenerate in "both" mode never shows the prior run\'s report against the new résumé', async () => {
    let resolveResume!: (v: string) => void;
    vi.mocked(generateResume).mockImplementationOnce(
      () => new Promise((resolve) => (resolveResume = resolve))
    );
    const { handleGenerate, m } = setup('both');

    const run = handleGenerate();
    // handleGenerate clears state — including the report — synchronously,
    // before ever awaiting the résumé generation call.
    await vi.waitFor(() => expect(generateResume).toHaveBeenCalled());
    expect(m.setReport).toHaveBeenCalledWith(null);
    // Still mid-flight: the résumé hasn't resolved, so stage hasn't reached
    // 'done' (the progressive-reveal window) yet — the report was already
    // cleared well before that window opens.
    expect(stageCalls(m)).not.toContain('done');

    resolveResume('RESUME');
    await run;
  });
});

describe('useGeneration — stale-persist guard (Regenerate/reset during validation)', () => {
  const STALE_REPORT = {
    schemaVersion: 2 as const,
    pipeline: 'fast' as const,
    generatedAt: 1,
    resume: {
      report: {
        ok: true,
        issues: [],
        metrics: {
          keywordCoverage: null,
          topRequirementHits: 0,
          duplicateRatio: 0,
          rolesSource: 0,
          rolesOutput: 0,
        },
      },
      sourceTextHash: 1,
    },
  };

  it('a Regenerate click during validation never persists the superseded run', async () => {
    // First call to computeQualityReport (the run about to be superseded) hangs
    // until we resolve it late; every later call resolves immediately.
    let resolveStale!: (r: typeof STALE_REPORT) => void;
    vi.mocked(computeQualityReport).mockImplementationOnce(
      () => new Promise((resolve) => (resolveStale = resolve))
    );
    const { handleGenerate, m } = setup('resume');

    const first = handleGenerate();
    await vi.waitFor(() => expect(computeQualityReport).toHaveBeenCalledTimes(1));

    // Regenerate clicked while the first run is still validating.
    const second = handleGenerate();
    await second;

    // The stale run's validation finally resolves — must be a no-op.
    resolveStale(STALE_REPORT);
    await first;

    // Both runs eagerly clear the report at their own top (2 calls) and the
    // winning (second) run's persist() clears it again once validation
    // resolves null (3rd call) — the superseded run's OWN persist() bails on
    // its aborted-controller check before ever reaching setReport, so no call
    // ever carries the stale run's report. That "never persists twice" is
    // what the save/notify counts below prove.
    expect(m.setReport).toHaveBeenCalledTimes(3);
    expect(m.setReport).toHaveBeenLastCalledWith(null);
    expect(m.saveAiGeneration.mutate).toHaveBeenCalledTimes(1);
    expect(m.notify.success).toHaveBeenCalledTimes(1);
  });

  it('an abort during validation (Reset) skips setReport/save for that run', async () => {
    let resolveStale!: (r: typeof STALE_REPORT) => void;
    vi.mocked(computeQualityReport).mockImplementationOnce(
      () => new Promise((resolve) => (resolveStale = resolve))
    );
    const { handleGenerate, m, abortControllerRef } = setup('resume');

    const run = handleGenerate();
    await vi.waitFor(() => expect(computeQualityReport).toHaveBeenCalledTimes(1));

    // Simulate what AIGeneratePage's reset() now does unconditionally.
    abortControllerRef.current?.abort();

    resolveStale(STALE_REPORT);
    await run;

    // The run's own top-of-function clear still fires once (setReport(null) —
    // never a stale report). Its persist() then bails before EVER calling
    // setReport/mutate again once resolved, and its finally() owns the ref
    // (nothing else replaced it) so it clears it back to null.
    expect(m.setReport).toHaveBeenCalledTimes(1);
    expect(m.setReport).toHaveBeenCalledWith(null);
    expect(m.saveAiGeneration.mutate).not.toHaveBeenCalled();
    expect(m.notify.success).not.toHaveBeenCalled();
    expect(abortControllerRef.current).toBeNull();
  });
});

describe('useGeneration — supersede while a stream is in flight (catch race)', () => {
  it('a Regenerate click during the cover-letter stream leaves the newer run untouched when the stale stream later rejects', async () => {
    // Run #1's cover-letter stream hangs until we reject it late; run #2's own
    // cover-letter call also hangs, so run #2 is still mid-flight (not yet
    // finished) when run #1's stale rejection lands.
    let rejectRun1Cover!: (err: Error) => void;
    let resolveRun2Cover!: (r: { text: string; companyBrief: string }) => void;
    vi.mocked(generateCoverLetter)
      .mockImplementationOnce(() => new Promise((_resolve, reject) => (rejectRun1Cover = reject)))
      .mockImplementationOnce(() => new Promise((resolve) => (resolveRun2Cover = resolve)));
    const { handleGenerate, m } = setup('both');

    const first = handleGenerate();
    // Résumé already revealed (progressive reveal); run #1's cover letter is
    // mid-stream.
    await vi.waitFor(() => expect(generateCoverLetter).toHaveBeenCalledTimes(1));

    // Regenerate clicked while run #1's cover letter is still streaming — this
    // aborts run #1's controller and takes over the ref.
    const second = handleGenerate();
    // Run #2's own résumé resolves and it reaches its own cover-letter call,
    // still mid-flight.
    await vi.waitFor(() => expect(generateCoverLetter).toHaveBeenCalledTimes(2));

    const stageCallsBefore = m.setStage.mock.calls.length;
    const stopRotationCallsBefore = m.stopStageRotation.mock.calls.length;
    const streamBufferCallsBefore = m.setStreamBuffer.mock.calls.length;
    const genStepCallsBefore = m.setGenStep.mock.calls.length;

    // Run #1's aborted stream finally rejects — its own supersession, nothing
    // to do with run #2's still-in-flight generation.
    rejectRun1Cover(new Error('cover boom — stale'));
    await first;

    // The superseded run's catch must be a no-op: none of run #2's in-flight
    // UI state gets stomped by run #1's late rejection.
    expect(m.setStage.mock.calls.length).toBe(stageCallsBefore);
    expect(m.stopStageRotation.mock.calls.length).toBe(stopRotationCallsBefore);
    expect(m.setStreamBuffer.mock.calls.length).toBe(streamBufferCallsBefore);
    expect(m.setGenStep.mock.calls.length).toBe(genStepCallsBefore);
    expect(stageCalls(m).at(-1)).toBe('done'); // still run #2's progressive reveal
    expect(m.setIsGenerating).toHaveBeenLastCalledWith(true); // run #2 still in flight

    // Clean up run #2 so no promise is left dangling past the test.
    resolveRun2Cover({ text: 'COVER', companyBrief: 'BRIEF' });
    await second;
  });
});
