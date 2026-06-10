import { create } from 'zustand';

import {
  extractMetadata,
  generateCoverLetter,
  generateResume,
  type GenerationMeta,
  type GenerationMode,
} from '@/lib/generate';

/** Which document(s) a run produces. */
export type TailorTarget = 'resume' | 'cover' | 'both';

/** Coarse generation phase, surfaced to the UI. */
export type GenerationPhase = 'idle' | 'analyzing' | 'resume' | 'cover';

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
};

/** The finished documents + detected metadata, handed to {@link RunTailorParams.onComplete}. */
export interface GenerationResult {
  meta: GenerationMeta;
  resumeText: string;
  coverLetterText: string;
}

export interface RunTailorParams {
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
    }) => {
      if (get().sessions[id]?.generating || !resume.trim()) return;

      const controller = new AbortController();
      controllers.set(id, controller);

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
          coverLetterText = await generateCoverLetter(
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
          patch(id, { coverOut: coverLetterText });
        }

        // Persist after a clean run only — a cancel/error throws and skips this.
        onComplete?.({ meta: detected, resumeText, coverLetterText });
      } catch (err) {
        // A user cancel aborts the controller — don't surface that as an error.
        if (!controller.signal.aborted) {
          patch(id, { error: err instanceof Error ? err.message : t('autopilot.apply.failed') });
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
