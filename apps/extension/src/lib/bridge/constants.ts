/** Inclusive probe range — mirrors the desktop `PORT_RANGE` (47615..=47620). */
export const PORT_START = 47615;
export const PORT_END = 47620;

/** Native host name — MUST match the Rust `extension_bridge::mod::NATIVE_HOST_NAME`. */
export const HOST_NAME = 'app.aijobhunter.bridge';

/** Per-request timeout (ms) — the desktop fetch+parse for URL mode can be slow. */
export const REQUEST_TIMEOUT_MS = 30_000;

/**
 * Stall timeout (ms) for a streaming `answerAssist` call — RESET on every
 * `assist.chunk`, unlike the flat {@link REQUEST_TIMEOUT_MS} every other verb
 * uses. A multi-second draft is normal (the desktop is mid-generation, still
 * emitting deltas), so the promise must stay alive as long as chunks keep
 * arriving; only genuine SILENCE — no chunk, no terminal reply — for this long
 * is a stall.
 */
export const ASSIST_STALL_TIMEOUT_MS = 60_000;

/** Backoff schedule (ms) for reconnect attempts; the last value repeats. */
export const BACKOFF_MS = [500, 1_000, 2_000, 5_000, 10_000];

/** WS handshake/open timeout per port probe. */
export const OPEN_TIMEOUT_MS = 1_500;

/**
 * Per-step timeout for the v2 handshake (await challenge / await auth.ok). On
 * loopback each step is a fast round-trip; a total silence (no frame, no close)
 * is treated as a transient transport failure (reconnect), while an explicit
 * close / a non-`challenge` reply is the outdated-desktop signal.
 */
export const HANDSHAKE_TIMEOUT_MS = 8_000;

/** How long to wait for the native host's `bridge.ready` before falling back to ws. */
export const READY_TIMEOUT_MS = 1_500;
