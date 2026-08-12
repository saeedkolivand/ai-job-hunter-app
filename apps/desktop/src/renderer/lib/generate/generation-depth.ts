/**
 * How much work one résumé generation does — the renderer's half of the shared
 * `GenerationDepth` vocabulary.
 *
 * `fast` is today's single-shot path (unchanged, byte-for-byte); `quality` is
 * the staged Rust pipeline (`resumePipeline.run`); `max` is Phase 4's
 * section-wise generator. The vocabulary itself lives in `@ajh/shared` because
 * Rust reads the same list through `pnpm gen:ipc`.
 */
import { GENERATION_DEPTHS, type GenerationDepth } from '@ajh/shared/schemas';

/**
 * Depths this build can actually RUN, in ascending order.
 *
 * All three run as of Phase 4: `resume_pipeline_run` no longer rejects
 * `depth: "max"` — it routes to the section-wise pipeline
 * (`MAX_STAGES = analyze_job, match_evidence, strategy, sections, assemble,
 * validate, repair, llm_judge`), so offering it is now a true claim about what
 * would run. `fast` is the only depth that never reaches the staged pipeline at
 * all; it stays here because it is a depth this build runs, just through the
 * one-shot path.
 *
 * The list stays a subset check rather than "everything in `GENERATION_DEPTHS`"
 * because the vocabulary is shared with Rust and codegen'd: a tier added to the
 * wire enum before this build can run it must be SHOWN (disabled, with a
 * reason) rather than silently offered — a missing tier reads as "there are
 * only N depths", which is a different and less honest claim than "there are
 * N+1, one isn't ready".
 */
export const RUNNABLE_GENERATION_DEPTHS: readonly GenerationDepth[] = ['fast', 'quality', 'max'];

export function isDepthRunnable(depth: GenerationDepth): boolean {
  return RUNNABLE_GENERATION_DEPTHS.includes(depth);
}

/**
 * Coerce a stored/derived depth to one this build can run.
 *
 * Falls back to `'fast'` — the cheapest depth and the app's default — never to
 * a staged one: silently upgrading someone into a multi-call staged run
 * (quality) or a per-section fan-out that may take the better part of an hour
 * (max) because their stored value named a tier this build doesn't have is the
 * one wrong answer here. Every current tier IS runnable, so the coercion is
 * unreachable today from both the UI and a well-formed persisted value; it
 * still covers a hand-edited or FORWARD-migrated one (a newer build's depth
 * read back by an older one), which is exactly when guessing upward would cost
 * the most.
 */
export function resolveRunnableDepth(depth: GenerationDepth | undefined | null): GenerationDepth {
  return depth && isDepthRunnable(depth) ? depth : 'fast';
}

/** The full vocabulary, for surfaces that DESCRIBE every depth (the info popover). */
export const ALL_GENERATION_DEPTHS = GENERATION_DEPTHS;

export type { GenerationDepth };
