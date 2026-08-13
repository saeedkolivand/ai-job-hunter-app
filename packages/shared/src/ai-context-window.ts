/**
 * The context-window (`num_ctx`) bounds every surface must agree on.
 *
 * SOURCE OF TRUTH for the Rust `MIN_CONTEXT_WINDOW` / `MAX_CONTEXT_WINDOW`,
 * which `pnpm gen:ipc` writes into `ipc_contracts::context_window` from the
 * values here. So the range the backend validator enforces and the range a
 * slider offers are one definition, and `gen:ipc:check` fails CI if they drift
 * — which is the failure this exists to prevent: a UI that accepts a value the
 * backend then rejects, or (worse) a UI that quietly clamps to a range the
 * backend has since widened.
 *
 * WHY these numbers: below ~512 a window cannot hold a system prompt plus any
 * useful input, so the call is guaranteed to truncate; above 131072 the request
 * stops being a size and becomes an out-of-memory kill of the user's own
 * machine, because Ollama allocates `num_ctx` up front.
 *
 * NOT a UI floor. A surface may impose a STRICTER minimum for its own reasons
 * (the local-model limits panel starts at 2048, because a local model with a
 * 512-token window is technically valid and practically useless). Those stay
 * local to the surface that chose them; only the contract lives here.
 */
export const CONTEXT_WINDOW_MIN = 512;

/** @see CONTEXT_WINDOW_MIN */
export const CONTEXT_WINDOW_MAX = 131_072;

/**
 * What a picker should start at when the user has set nothing — a middle
 * ground that fits every model this app recommends, NOT a value the backend
 * substitutes. An absent window means the provider's own default; the backend
 * never guesses a size.
 */
export const CONTEXT_WINDOW_DEFAULT = 8_192;

/** Whether a number is a context window the backend will accept. The runtime
 *  form of the bounds above, so a caller validates against the contract rather
 *  than re-deriving the comparison. */
export function isValidContextWindow(value: number): boolean {
  return Number.isInteger(value) && value >= CONTEXT_WINDOW_MIN && value <= CONTEXT_WINDOW_MAX;
}
