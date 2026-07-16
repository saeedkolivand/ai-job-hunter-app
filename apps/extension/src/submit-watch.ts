/**
 * Auto-track submit-watcher injected entry (compiled to `submit-watch.js`).
 *
 * Injected via `chrome.scripting.executeScript({ files: ['submit-watch.js'] })`
 * right after any existing page gesture (autofill / answer-fill / answers-
 * capture / import scan) WHEN the auto-track opt-in is on — see
 * `background.ts`'s `maybeArmSubmitWatch`. Runs as a CLASSIC script (no ES
 * modules), so after the isolated Rollup pass (`vite.config.ts`'s
 * `injectedEntries`) it carries ZERO `import` statements; its only runtime
 * import is the pure `./lib/submit-watch`, inlined by that pass.
 *
 * Idempotent: a page can be gestured (and this re-injected) many times, so it
 * arms the watcher AT MOST ONCE per frame via an isolated-world global flag
 * (same discipline as `AUTOFILL_GLOBAL`). The one extension API it touches is
 * `chrome.runtime.sendMessage` (available in the injected isolated world), used
 * to post the detected URL back to the background — fire-and-forget.
 */

import { armSubmitWatch, SUBMIT_DETECTED_MSG } from './lib/submit-watch';

/** Isolated-world arm-once flag: re-injection after another gesture on the same
 *  frame must not stack a second listener set. */
const ARMED_FLAG = '__ajhSubmitWatchArmed';

// `chrome` is available in the injected isolated-world content-script context in
// both Chrome and Firefox; declared locally so this classic script pulls in no
// extension-types dependency.
declare const chrome: { runtime: { sendMessage(message: unknown): unknown } };

const g = globalThis as unknown as Record<string, boolean>;
if (!g[ARMED_FLAG]) {
  g[ARMED_FLAG] = true;
  armSubmitWatch(document, (url) => {
    try {
      // Fire-and-forget. In MV3 (and Firefox) `sendMessage` returns a Promise
      // that rejects when the background isn't listening (SW asleep) — swallow
      // both that async rejection and any synchronous "context invalidated" throw.
      const maybePromise = chrome.runtime.sendMessage({ kind: SUBMIT_DETECTED_MSG, url });
      if (maybePromise && typeof (maybePromise as { catch?: unknown }).catch === 'function') {
        (maybePromise as Promise<unknown>).catch(() => {});
      }
    } catch {
      // Background unavailable — best-effort only.
    }
  });
}

// Ensure this file is treated as an ES module.
export {};
