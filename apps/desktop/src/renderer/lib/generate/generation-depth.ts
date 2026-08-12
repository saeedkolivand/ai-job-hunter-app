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
 * `max` is deliberately absent, and this is checked against the backend rather
 * than assumed: `resume_pipeline_run` rejects `depth: "max"` with a validation
 * error ("the staged pipeline runs at quality depth"), because accepting it and
 * running the quality stages would be a lie about what ran. The option is still
 * SHOWN (disabled, with a reason) rather than hidden — a missing third tier
 * reads as "there are two depths", which is a different and less honest claim
 * than "there are three, one isn't ready".
 */
export const RUNNABLE_GENERATION_DEPTHS: readonly GenerationDepth[] = ['fast', 'quality'];

export function isDepthRunnable(depth: GenerationDepth): boolean {
  return RUNNABLE_GENERATION_DEPTHS.includes(depth);
}

/**
 * Coerce a stored/derived depth to one this build can run.
 *
 * Falls back to `'fast'` — the cheapest depth and the app's default — never to
 * `'quality'`: silently upgrading someone into four provider calls plus two
 * repair rounds because their stored value named a tier that doesn't exist yet
 * is the one wrong answer here. Unreachable through the UI (the control refuses
 * to select an unrunnable option); this covers a hand-edited or
 * forward-migrated persisted value.
 */
export function resolveRunnableDepth(depth: GenerationDepth | undefined | null): GenerationDepth {
  return depth && isDepthRunnable(depth) ? depth : 'fast';
}

/** The full vocabulary, for surfaces that DESCRIBE every depth (the info popover). */
export const ALL_GENERATION_DEPTHS = GENERATION_DEPTHS;

export type { GenerationDepth };
