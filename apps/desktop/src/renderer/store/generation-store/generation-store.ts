import { create } from 'zustand';

import { errorDetail } from '@/lib/error-class';
import {
  computeQualityReport,
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
  type QualityReport,
} from '@/lib/generate';
import { resolveActiveProvider } from '@/lib/generate/provider-context';

/** Which document(s) a run produces. */
export type TailorTarget = 'resume' | 'cover' | 'both';

/** Coarse generation phase, surfaced to the UI. */
type GenerationPhase = 'idle' | 'analyzing' | 'resume' | 'cover';

/**
 * One generation session's durable state. Keyed by a caller-supplied **context
 * id** (e.g. `autopilot:<jobUrl>`), this lives in the store rather than a
 * component so closing a modal or navigating away preserves the result and lets
 * generation finish in the background — the desktop behaviour the app wants.
 */
export interface GenerationSession {
  generating: boolean;
  phase: GenerationPhase;
  resumeOut: string;
  coverOut: string;
  thinking: string;
  activeOut: 'resume' | 'cover';
  error: string | null;
  meta: GenerationMeta | null;
  /** Persisted record id once {@link RunTailorParams.onComplete} has saved this
   *  session's result — lets the modal persist later edits to the right record. */
  savedId: string | null;
  /** Deterministic content-quality report for the most recent run (ADR-007
   *  addendum) — additive sibling to `meta`; never read by `phase`. */
  report: QualityReport | null;
}

/** Stable empty session — returned for unknown ids so selectors keep one reference. */
export const EMPTY_SESSION: GenerationSession = {
  generating: false,
  phase: 'idle',
  resumeOut: '',
  coverOut: '',
  thinking: '',
  activeOut: 'cover',
  error: null,
  meta: null,
  savedId: null,
  report: null,
};

/** The finished documents + detected metadata, handed to {@link RunTailorParams.onComplete}. */
export interface GenerationResult {
  meta: GenerationMeta;
  resumeText: string;
  coverLetterText: string;
  /** Company-research brief that informed the cover letter (`''` when research was
   *  off or no cover letter was generated). Persisted so the doc card can show the
   *  "Company research" section. */
  companyBrief: string;
  /** Deterministic content-quality report for this run, or `null` if neither
   *  doc validated (see `computeQualityReport`) — the caller persists it. */
  report: QualityReport | null;
}

interface RunTailorParams {
  contextId: string;
  resume: string;
  jobDesc: string;
  model: string;
  mode: GenerationMode;
  target: TailorTarget;
  /** Opt-in: research the company and fold a brief into the cover-letter prompt. */
  researchCompany?: boolean;
  /** Translator for the failure message. */
  t: (key: string) => string;
  /**
   * Called once after a run completes successfully (not on cancel/error). The
   * store stays a pure state container — persistence (e.g. saving the application
   * record) lives in the caller. Fires even if the originating component has
   * unmounted, so a background generation still records its result.
   */
  onComplete?: (result: GenerationResult) => void;
  /**
   * Called once when a run fails (not on cancel) — lets the caller fire a toast.
   * The failure message itself is written to the session's `error` field
   * regardless, so the UI can render it inline even if the caller has unmounted.
   */
  onError?: () => void;
}

interface GenerationStore {
  sessions: Record<string, GenerationSession>;
  /** Current session for a context id (or the stable empty session). */
  getSession: (id: string) => GenerationSession;
  setActiveOut: (id: string, which: 'resume' | 'cover') => void;
  /** Replace one document's text in a session (e.g. an inline edit/rewrite). */
  setOutput: (id: string, which: 'resume' | 'cover', text: string) => void;
  /** Record the persisted id after a clean run saves the session's result. */
  setSavedId: (id: string, savedId: string) => void;
  /**
   * Seed a session from a persisted record so the flow opens on the results
   * (`done`) stage instead of the wizard. No-clobber + idempotent: writes ONLY
   * when the session is empty (not generating, no output, unsaved), so it is
   * safe to call from a render-effect — repeat calls and in-flight/edited
   * sessions are left untouched.
   */
  hydrate: (
    id: string,
    seed: Pick<GenerationSession, 'resumeOut' | 'coverOut' | 'meta' | 'savedId'> &
      Partial<Pick<GenerationSession, 'report'>>
  ) => void;
  /** Cancel an in-flight run for a context id. */
  cancel: (id: string) => void;
  /** Drop a session entirely. */
  reset: (id: string) => void;
  /**
   * Run analyze → resume → cover for a context, writing progress to the store.
   * Decoupled from any component lifecycle, so it survives unmount/navigation.
   */
  runTailor: (params: RunTailorParams) => Promise<void>;
}

// AbortControllers are non-serializable and lifecycle-independent, so they live
// in a module map keyed by context id rather than in the store state.
const controllers = new Map<string, AbortController>();

export const useGenerationStore = create<GenerationStore>((set, get) => {
  const patch = (id: string, partial: Partial<GenerationSession>) =>
    set((state) => ({
      sessions: {
        ...state.sessions,
        [id]: { ...(state.sessions[id] ?? EMPTY_SESSION), ...partial },
      },
    }));

  // Append-by-read so concurrent token + thinking deltas never clobber each other
  // with a stale closure value.
  const append = (id: string, key: 'resumeOut' | 'coverOut' | 'thinking', tok: string) =>
    patch(id, { [key]: (get().sessions[id]?.[key] ?? '') + tok });

  return {
    sessions: {},

    getSession: (id) => get().sessions[id] ?? EMPTY_SESSION,

    setActiveOut: (id, which) => patch(id, { activeOut: which }),

    setOutput: (id, which, text) =>
      patch(id, which === 'resume' ? { resumeOut: text } : { coverOut: text }),

    setSavedId: (id, savedId) => patch(id, { savedId }),

    hydrate: (id, seed) =>
      set((state) => {
        const existing = state.sessions[id] ?? EMPTY_SESSION;
        // Never clobber an in-flight run, existing output, or a saved session —
        // the live session (or the user's in-progress work) always wins.
        if (existing.generating || existing.resumeOut || existing.coverOut || existing.savedId) {
          return state;
        }
        return {
          sessions: {
            ...state.sessions,
            [id]: {
              ...EMPTY_SESSION,
              resumeOut: seed.resumeOut,
              coverOut: seed.coverOut,
              meta: seed.meta,
              savedId: seed.savedId,
              report: seed.report ?? null,
              activeOut: seed.resumeOut ? 'resume' : 'cover',
            },
          },
        };
      }),

    cancel: (id) => {
      controllers.get(id)?.abort();
      controllers.delete(id);
      // Deterministically return to the pre-run state so the UI leaves the generating
      // stage on click. The abort above stops the in-flight stream (streamGenerate's
      // abort listener does off()/jobs.cancel); resetting here guarantees the UI
      // responds even if the abort lands in streamGenerate's pre-stream request window.
      patch(id, { ...EMPTY_SESSION });
    },

    reset: (id) =>
      set((state) => {
        if (!(id in state.sessions)) return state;
        const next = { ...state.sessions };
        delete next[id];
        return { sessions: next };
      }),

    runTailor: async ({
      contextId: id,
      resume,
      jobDesc,
      model,
      mode,
      target,
      researchCompany,
      t,
      onComplete,
      onError,
    }) => {
      if (get().sessions[id]?.generating || !resume.trim()) return;

      const controller = new AbortController();
      controllers.set(id, controller);

      // Resolved model, not the caller's `model` argument — see the same note
      // in `useGeneration`'s handleGenerate.
      const { activeProvider: provider, activeModel } = resolveActiveProvider(model);
      const startedAt = Date.now();
      console.warn('[runTailor] start', { provider, model: activeModel, target });

      // Fresh session for this run.
      patch(id, {
        ...EMPTY_SESSION,
        generating: true,
        phase: 'analyzing',
        activeOut: target === 'resume' ? 'resume' : 'cover',
      });

      const onThink = (tok: string) => append(id, 'thinking', tok);

      try {
        const detected = await extractMetadata(resume, jobDesc, model);
        patch(id, { meta: detected });

        let resumeText = '';
        let coverLetterText = '';
        // Carried into the saved record so the doc card's "Company research"
        // section shows for tailored cover letters (empty for resume-only runs).
        let companyBrief = '';

        if (target === 'resume' || target === 'both') {
          patch(id, { activeOut: 'resume', phase: 'resume', thinking: '' });
          resumeText = await generateResume(
            resume,
            jobDesc,
            detected,
            mode,
            model,
            (tok) => append(id, 'resumeOut', tok),
            'en',
            controller.signal,
            onThink
          );
          patch(id, { resumeOut: resumeText });
        }

        if (target === 'cover' || target === 'both') {
          patch(id, { activeOut: 'cover', phase: 'cover', thinking: '' });
          const cover = await generateCoverLetter(
            resume,
            jobDesc,
            detected,
            mode,
            model,
            (tok) => append(id, 'coverOut', tok),
            'en',
            controller.signal,
            onThink,
            { researchCompany }
          );
          coverLetterText = cover.text;
          companyBrief = cover.companyBrief;
          patch(id, { coverOut: coverLetterText });
        }

        // For a combined run, land the results on the résumé tab (cover was generated
        // last, leaving activeOut on 'cover'); the user expects résumé first.
        if (target === 'both') patch(id, { activeOut: 'resume' });

        console.warn('[runTailor] done', {
          target,
          resumeLength: resumeText.length,
          coverLength: coverLetterText.length,
          durationMs: Date.now() - startedAt,
        });

        // Best-effort, never throws — see `computeQualityReport`. Stored on the
        // session (drives the results-panel badge) AND handed to `onComplete` so
        // the caller's save carries it.
        const report = await computeQualityReport({
          sourceResume: resume,
          jobAd: jobDesc,
          topRequirements: detected.topRequirements,
          targetLanguage: detected.targetLanguage,
          resumeText,
          coverLetterText,
        });
        patch(id, { report });

        // Persist after a clean run only — a cancel/error throws and skips this.
        onComplete?.({ meta: detected, resumeText, coverLetterText, companyBrief, report });
      } catch (err) {
        // A user cancel aborts the controller — don't surface that as an error.
        if (controller.signal.aborted) {
          // Still log it: without this, a cancelled run is indistinguishable in
          // the log from a run that hung or died mid-stream — both just stop
          // after '[runTailor] start'.
          console.warn('[runTailor] cancelled', {
            provider,
            model: activeModel,
            target,
            durationMs: Date.now() - startedAt,
          });
        } else {
          const message = err instanceof Error ? err.message : t('autopilot.apply.failed');
          // Capped, not classed — see `errorDetail`: the provider's own error
          // text is the ONLY record of why a generation died.
          console.error('[runTailor] failed', { target, error: errorDetail(err) });
          patch(id, { error: message });
          onError?.();
        }
      } finally {
        controllers.delete(id);
        // Only flip generating off if THIS run still owns the session (a cancel may have
        // already reset it, or a newer run may have started).
        if (get().sessions[id]?.generating) patch(id, { generating: false, phase: 'idle' });
      }
    },
  };
});
