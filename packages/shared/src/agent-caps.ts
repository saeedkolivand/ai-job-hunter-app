/**
 * Char caps on the text an agent run may carry — the SINGLE source of truth for
 * both sides, in the shape `ai-timeouts.ts` established:
 *
 * - Backend: `apps/desktop/src-tauri/src/agent/tools.rs` (`RESUME_CAP`) is
 *   DEFINED as the generated `ipc_contracts::agent_caps` constant, emitted from
 *   this file by `packages/shared/scripts/gen-ipc-rust.ts` (`pnpm gen:ipc`).
 * - Renderer: the entry point that offers an `improve_resume` run imports this
 *   constant directly to decide whether the generation is short enough to
 *   review.
 *
 * It lived as a Rust literal and a hand-copied renderer literal in the same
 * change that added a generator for exactly this class of number. Two
 * independently-editable copies of a threshold whose halves must agree is the
 * drift `pnpm gen:ipc:check` exists to make impossible.
 */

/**
 * Longest résumé text, in CHARACTERS, that may cross into an agent run.
 *
 * It bounds three things that have to agree or the flow lies to someone:
 *
 * 1. the fence that seeds a run's transcript with a résumé
 *    (`agent::tools::fenced`, which clamps by `chars().take(cap)`);
 * 2. therefore the longest generation the `improve_resume` flow can READ — a
 *    longer one is refused at run start rather than silently truncated, because
 *    the gated save writes up to 40 000 chars back over the same row and would
 *    replace the document with the ~8 000-char stump the model actually saw;
 * 3. therefore the threshold the renderer disables the "improve" entry point on,
 *    so the user learns the document is too long BEFORE starting a run that can
 *    only fail.
 *
 * CHARACTERS, not bytes or UTF-16 units: it is compared against Rust's
 * `chars().count()` (Unicode scalar values). For the strings this bounds the
 * only visible difference from a JS `[...text].length` is astral-plane
 * characters, which count once in both.
 */
export const AGENT_RESUME_TEXT_CAP = 8_000;
