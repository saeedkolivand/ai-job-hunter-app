import { beforeEach, describe, expect, it, vi } from 'vitest';

import { computeQualityReport, generateResume } from '@/lib/generate';

import { EMPTY_SESSION, useGenerationStore } from './generation-store';

// Stub the generation pipeline; resume/cover emit one reasoning + one content token.
vi.mock('@/lib/generate', () => ({
  extractMetadata: vi.fn().mockResolvedValue({
    candidateName: '',
    jobTitle: '',
    companyName: '',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  }),
  generateResume: vi.fn(
    async (
      _r: string,
      _j: string,
      _m: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _l?: string,
      _s?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('R-think');
      onToken('RESUME');
      return 'RESUME';
    }
  ),
  generateCoverLetter: vi.fn(
    async (
      _r: string,
      _j: string,
      _m: unknown,
      _mode: string,
      _model: string,
      onToken: (t: string) => void,
      _l?: string,
      _s?: AbortSignal,
      onThinking?: (t: string) => void
    ) => {
      onThinking?.('C-think');
      onToken('COVER');
      return { text: 'COVER', companyBrief: 'BRIEF' };
    }
  ),
  computeQualityReport: vi.fn().mockResolvedValue(null),
}));

const t = (k: string) => k;
const base = { resume: 'r', jobDesc: 'jd', model: 'llama', mode: 'ats' as const, t };

beforeEach(() => {
  useGenerationStore.setState({ sessions: {} });
  vi.mocked(computeQualityReport).mockResolvedValue(null);
});

describe('generation store', () => {
  it('returns one stable empty session reference for unknown ids', () => {
    const s = useGenerationStore.getState();
    expect(s.getSession('nope')).toBe(EMPTY_SESSION);
    expect(s.getSession('other')).toBe(EMPTY_SESSION);
  });

  it('runs analyze → resume → cover into the session, surviving a remount', async () => {
    const id = 'autopilot:job-1';
    await useGenerationStore.getState().runTailor({ contextId: id, target: 'both', ...base });

    // A fresh getState read simulates a component remounting after navigation:
    // the session is still there, fully populated.
    const s = useGenerationStore.getState().getSession(id);
    expect(s.resumeOut).toBe('RESUME');
    expect(s.coverOut).toBe('COVER');
    expect(s.thinking).toBe('C-think'); // reasoning cleared at the cover phase
    expect(s.generating).toBe(false);
    expect(s.phase).toBe('idle');
    expect(s.meta).not.toBeNull();
    // Cover streams last but the results must open on the résumé tab.
    expect(s.activeOut).toBe('resume');
  });

  it('lands a single-target run on its own tab (resume → resume, cover → cover)', async () => {
    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'only-resume', target: 'resume', ...base });
    expect(useGenerationStore.getState().getSession('only-resume').activeOut).toBe('resume');

    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'only-cover', target: 'cover', ...base });
    expect(useGenerationStore.getState().getSession('only-cover').activeOut).toBe('cover');
  });

  it('keeps sessions isolated per context id', async () => {
    await useGenerationStore.getState().runTailor({ contextId: 'a', target: 'resume', ...base });
    expect(useGenerationStore.getState().getSession('a').resumeOut).toBe('RESUME');
    expect(useGenerationStore.getState().getSession('b')).toBe(EMPTY_SESSION);
  });

  it('ignores a concurrent run for a context that is already generating', async () => {
    const id = 'busy';
    useGenerationStore.setState({ sessions: { [id]: { ...EMPTY_SESSION, generating: true } } });
    await useGenerationStore.getState().runTailor({ contextId: id, target: 'resume', ...base });
    expect(useGenerationStore.getState().getSession(id).resumeOut).toBe(''); // guard returned early
  });

  it('reset drops a session', () => {
    const id = 'done';
    useGenerationStore.setState({ sessions: { [id]: { ...EMPTY_SESSION, resumeOut: 'X' } } });
    useGenerationStore.getState().reset(id);
    expect(useGenerationStore.getState().getSession(id)).toBe(EMPTY_SESSION);
  });

  it('calls onComplete with the finished documents + metadata after a clean run', async () => {
    const onComplete = vi.fn();
    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'c1', target: 'both', onComplete, ...base });

    expect(onComplete).toHaveBeenCalledTimes(1);
    // The cover-letter brief threads through to onComplete so the caller persists it.
    expect(onComplete).toHaveBeenCalledWith({
      meta: expect.objectContaining({ resumeLanguage: 'en' }),
      resumeText: 'RESUME',
      coverLetterText: 'COVER',
      companyBrief: 'BRIEF',
      report: null,
    });
  });

  it('passes an empty companyBrief to onComplete for a resume-only run', async () => {
    const onComplete = vi.fn();
    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'resume-only', target: 'resume', onComplete, ...base });

    expect(onComplete).toHaveBeenCalledWith(
      expect.objectContaining({ resumeText: 'RESUME', coverLetterText: '', companyBrief: '' })
    );
  });

  it('does not call onComplete when the run fails (so a failed application is never saved)', async () => {
    vi.mocked(generateResume).mockRejectedValueOnce(new Error('boom'));
    const onComplete = vi.fn();
    await useGenerationStore
      .getState()
      .runTailor({ contextId: 'c2', target: 'resume', onComplete, ...base });

    expect(onComplete).not.toHaveBeenCalled();
    expect(useGenerationStore.getState().getSession('c2').error).toBe('boom');
  });
});

describe('generation store — quality report', () => {
  it('stores the computed report on the session and hands it to onComplete', async () => {
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
    const onComplete = vi.fn();
    const id = 'quality-1';
    await useGenerationStore
      .getState()
      .runTailor({ contextId: id, target: 'resume', onComplete, ...base });

    expect(useGenerationStore.getState().getSession(id).report).toEqual(report);
    expect(onComplete).toHaveBeenCalledWith(expect.objectContaining({ report }));
  });

  it('degrades to a null report without failing the run when validation fails', async () => {
    vi.mocked(computeQualityReport).mockResolvedValueOnce(null);
    const onComplete = vi.fn();
    const id = 'quality-2';
    await useGenerationStore
      .getState()
      .runTailor({ contextId: id, target: 'resume', onComplete, ...base });

    expect(useGenerationStore.getState().getSession(id).report).toBeNull();
    expect(useGenerationStore.getState().getSession(id).error).toBeNull();
    expect(onComplete).toHaveBeenCalledWith(expect.objectContaining({ report: null }));
  });

  it('a cancel mid-validation never resurrects the reset session or fires onComplete', async () => {
    // Deferred validation call — lets us cancel() BETWEEN generation finishing
    // and the report landing, the exact window the reproduced bug lived in.
    let resolveReport!: (r: null) => void;
    // Prior tests share this module-level mock, so its call count is cumulative —
    // clear it so the waitFor below observes only THIS run's call.
    vi.mocked(computeQualityReport).mockClear();
    vi.mocked(computeQualityReport).mockImplementationOnce(
      () => new Promise((resolve) => (resolveReport = resolve))
    );
    const onComplete = vi.fn();
    const id = 'cancel-race';
    const run = useGenerationStore
      .getState()
      .runTailor({ contextId: id, target: 'resume', onComplete, ...base });

    await vi.waitFor(() => expect(computeQualityReport).toHaveBeenCalledTimes(1));
    // The session is mid-run (generating) right up to the cancel.
    expect(useGenerationStore.getState().getSession(id).generating).toBe(true);

    useGenerationStore.getState().cancel(id);
    // cancel() merges EMPTY_SESSION's fields into the existing session object
    // (patch never deletes the key), so this is value-equal, not the same
    // reference as the module's stable EMPTY_SESSION constant.
    expect(useGenerationStore.getState().getSession(id)).toEqual(EMPTY_SESSION);

    // Validation resolves LATE, after the cancel.
    resolveReport(null);
    await run;

    // The cancelled run must never resurrect the session or persist anything.
    expect(useGenerationStore.getState().getSession(id)).toEqual(EMPTY_SESSION);
    expect(onComplete).not.toHaveBeenCalled();
  });
});

describe('generation store — hydrate (cold-entry seed)', () => {
  const meta = {
    candidateName: 'Ada',
    jobTitle: 'Engineer',
    companyName: 'Acme',
    resumeLanguage: 'en',
    jobAdLanguage: 'en',
    mismatch: false,
    targetLanguage: 'en',
    topRequirements: [],
  };
  const seed = { resumeOut: 'SAVED-RESUME', coverOut: 'SAVED-COVER', meta, savedId: 'rec-1' };

  it('seeds an empty session and lands on the résumé tab when a résumé exists', () => {
    const id = 'autopilot:job-x';
    useGenerationStore.getState().hydrate(id, seed);

    const s = useGenerationStore.getState().getSession(id);
    expect(s.resumeOut).toBe('SAVED-RESUME');
    expect(s.coverOut).toBe('SAVED-COVER');
    expect(s.savedId).toBe('rec-1');
    expect(s.meta).toEqual(meta);
    expect(s.generating).toBe(false);
    expect(s.activeOut).toBe('resume');
    // Seed omitted `report` — defaults to null rather than leaving it undefined.
    expect(s.report).toBeNull();
  });

  it('seeds the report when the caller provides one (reopening an already-checked generation)', () => {
    const id = 'autopilot:job-y';
    const report = { schemaVersion: 2 as const, pipeline: 'fast' as const, generatedAt: 1 };
    useGenerationStore.getState().hydrate(id, { ...seed, report });
    expect(useGenerationStore.getState().getSession(id).report).toEqual(report);
  });

  it('lands on the cover tab when only a cover letter exists', () => {
    const id = 'cover-only';
    useGenerationStore.getState().hydrate(id, { ...seed, resumeOut: '' });
    expect(useGenerationStore.getState().getSession(id).activeOut).toBe('cover');
  });

  it('no-ops when a run is in flight (never clobbers the live session)', () => {
    const id = 'busy';
    useGenerationStore.setState({ sessions: { [id]: { ...EMPTY_SESSION, generating: true } } });
    useGenerationStore.getState().hydrate(id, seed);

    const s = useGenerationStore.getState().getSession(id);
    expect(s.resumeOut).toBe('');
    expect(s.generating).toBe(true);
  });

  it('no-ops when output already exists (live output wins over the saved record)', () => {
    const id = 'has-output';
    useGenerationStore.setState({ sessions: { [id]: { ...EMPTY_SESSION, resumeOut: 'LIVE' } } });
    useGenerationStore.getState().hydrate(id, seed);
    expect(useGenerationStore.getState().getSession(id).resumeOut).toBe('LIVE');
  });

  it('no-ops when the session is already saved (savedId set)', () => {
    const id = 'already-saved';
    useGenerationStore.setState({
      sessions: { [id]: { ...EMPTY_SESSION, savedId: 'other-rec' } },
    });
    useGenerationStore.getState().hydrate(id, seed);

    const s = useGenerationStore.getState().getSession(id);
    expect(s.savedId).toBe('other-rec');
    expect(s.resumeOut).toBe('');
  });

  it('is idempotent — a second hydrate call after seeding does not re-apply', () => {
    const id = 'idem';
    useGenerationStore.getState().hydrate(id, seed);
    useGenerationStore.getState().hydrate(id, { ...seed, resumeOut: 'DIFFERENT' });
    // The first seed set savedId, so the second call is a no-op.
    expect(useGenerationStore.getState().getSession(id).resumeOut).toBe('SAVED-RESUME');
  });
});

describe('generation store — staged-pipeline siblings (ADR-006)', () => {
  beforeEach(() => useGenerationStore.setState({ sessions: {} }));

  it('starts every session with all three siblings null', () => {
    expect(EMPTY_SESSION.runId).toBeNull();
    expect(EMPTY_SESSION.pipelineStage).toBeNull();
    expect(EMPTY_SESSION.sectionStates).toBeNull();
  });

  it('patches one sibling without disturbing the other two', () => {
    const id = 'quality:job-1';
    const store = useGenerationStore.getState();
    store.setPipeline(id, { runId: 'run-9' });
    store.setPipeline(id, {
      pipelineStage: { stage: 'draft', phase: 'start', index: 3, total: 6, attempt: 1 },
    });
    store.setPipeline(id, { sectionStates: { summary: 'clean' } });

    const s = useGenerationStore.getState().getSession(id);
    expect(s.runId).toBe('run-9');
    expect(s.pipelineStage).toMatchObject({ stage: 'draft', index: 3 });
    expect(s.sectionStates).toEqual({ summary: 'clean' });
  });

  it('leaves the fast path byte-identical — a run writes no pipeline sibling', async () => {
    // The whole point of ADR-006 siblings: `GenerationPhase` is untouched and
    // the one-shot path behaves exactly as before. If `runTailor` ever starts
    // writing these, the quality surface would light up for a fast generation.
    const id = 'fast:job-1';
    await useGenerationStore.getState().runTailor({ contextId: id, target: 'resume', ...base });

    const s = useGenerationStore.getState().getSession(id);
    expect(s.resumeOut).toBe('RESUME');
    expect(s.runId).toBeNull();
    expect(s.pipelineStage).toBeNull();
    expect(s.sectionStates).toBeNull();
  });

  it('clears the siblings when a session is cancelled', () => {
    const id = 'quality:job-2';
    const store = useGenerationStore.getState();
    store.setPipeline(id, { runId: 'run-9', sectionStates: { skills: 'needsChanges' } });
    store.cancel(id);

    const s = useGenerationStore.getState().getSession(id);
    expect(s.runId).toBeNull();
    expect(s.sectionStates).toBeNull();
  });
});
