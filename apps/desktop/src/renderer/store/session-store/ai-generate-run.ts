import { create } from 'zustand';

import { useSessionStore } from './session-store';

/**
 * Live AI Generate run state (AGENTS.md rule 16). Its own small store, not the
 * session slice: the renderer-driven stream writes it on every token, and pages
 * that read the whole session store must not re-render per token. Only the final
 * résumé/letter text is committed to the session slice.
 */
export interface AIGenerateRunStoreState {
  isGenerating: boolean;
  stageLabel: string;
  streamBuffer: string;
  thinkingBuffer: string;
  modelLoading: boolean;
  tokenCount: number;
  genStep: { current: number; total: number; label: string } | null;
  error: string | null;
  /** Per-token streamed output; empty once the final text is committed. */
  liveResume: string;
  liveCover: string;
}

const RUN_DEFAULTS: AIGenerateRunStoreState = {
  isGenerating: false,
  stageLabel: '',
  streamBuffer: '',
  thinkingBuffer: '',
  modelLoading: false,
  tokenCount: 0,
  genStep: null,
  error: null,
  liveResume: '',
  liveCover: '',
};

export const useAIGenerateRunStore = create<AIGenerateRunStoreState>(() => ({ ...RUN_DEFAULTS }));

// Module-level so a remounted page (or another route) shares the in-flight run's
// abort handle, token clock and stage timer; a per-mount ref would be orphaned.
export const runAbortRef: { current: AbortController | null } = { current: null };
export const runTokenStartRef: { current: number | null } = { current: null };
export const runStageTimer: { current: ReturnType<typeof setInterval> | null } = { current: null };

/** Abort any live run, stop its stage timer, and clear both the run and session state. */
export function resetAIGenerateAll() {
  runAbortRef.current?.abort();
  // Nulled so the aborted run's late catch/finally see they no longer own the page.
  runAbortRef.current = null;
  if (runStageTimer.current) clearInterval(runStageTimer.current);
  runStageTimer.current = null;
  useAIGenerateRunStore.setState({ ...RUN_DEFAULTS });
  useSessionStore.getState().resetAIGenerate();
}
