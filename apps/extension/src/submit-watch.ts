/**
 * Auto-track submit-watcher injected entry (compiled to `submit-watch.js`).
 *
 * Two-step injection (PR4, same `files` + `func` pattern as `fill.js`/
 * `AUTOFILL_GLOBAL`): `files: ['submit-watch.js']` registers
 * {@link SUBMIT_WATCH_GLOBAL} on the page global, then a self-contained
 * `func` invokes it with the `captureAnswers` boolean — the ONLY way to pass
 * an arm-time argument to a classic-script injection, and load-bearing here:
 * the save-answers-on-submit consent flag must cross the injection boundary
 * as a plain JSON-safe primitive, never something the page itself decides.
 * `background.ts`'s `maybeArmSubmitWatch` call site does this right after any
 * existing page gesture (autofill / answer-fill / answers-capture / import
 * scan) WHEN the auto-track opt-in is on. Runs as a CLASSIC script (no ES
 * modules), so after the isolated Rollup pass (`vite.config.mts`'s
 * `injectedEntries`) it carries ZERO `import` statements; its only runtime
 * import is the pure `./lib/submit-watch`, inlined by that pass.
 *
 * Idempotent: a page can be gestured (and this re-injected) many times, so it
 * arms the watcher AT MOST ONCE per frame via an isolated-world global flag
 * (same discipline as `AUTOFILL_GLOBAL`) — a later re-arm with a DIFFERENT
 * `captureAnswers` value is a no-op, same as every other arm-time setting
 * here (the watcher already fires at most once per frame regardless). The one
 * extension API it touches is `chrome.runtime.sendMessage` (available in the
 * injected isolated world), used to post the detected URL (plus any captured
 * answers) back to the background — fire-and-forget.
 */

import type { CapturedAnswer } from './lib/answers-capture';
import { armSubmitWatch, SUBMIT_DETECTED_MSG, SUBMIT_WATCH_GLOBAL } from './lib/submit-watch';

/** Isolated-world arm-once flag: re-injection after another gesture on the same
 *  frame must not stack a second listener set. */
const ARMED_FLAG = '__ajhSubmitWatchArmed';

// `chrome` is available in the injected isolated-world content-script context in
// both Chrome and Firefox; declared locally so this classic script pulls in no
// extension-types dependency.
declare const chrome: { runtime: { sendMessage(message: unknown): unknown } };

function runArmSubmitWatch(captureAnswers: boolean): void {
  const g = globalThis as unknown as Record<string, boolean>;
  if (g[ARMED_FLAG]) return;
  g[ARMED_FLAG] = true;
  armSubmitWatch(
    document,
    (url, answers) => {
      try {
        const message: { kind: string; url: string; answers?: CapturedAnswer[] } = {
          kind: SUBMIT_DETECTED_MSG,
          url,
        };
        if (answers) message.answers = answers;
        // Fire-and-forget. In MV3 (and Firefox) `sendMessage` returns a Promise
        // that rejects when the background isn't listening (SW asleep) — swallow
        // both that async rejection and any synchronous "context invalidated" throw.
        const maybePromise = chrome.runtime.sendMessage(message);
        if (maybePromise && typeof (maybePromise as { catch?: unknown }).catch === 'function') {
          (maybePromise as Promise<unknown>).catch(() => {});
        }
      } catch {
        // Background unavailable — best-effort only.
      }
    },
    { captureAnswers }
  );
}

(globalThis as unknown as Record<string, (captureAnswers: boolean) => void>)[SUBMIT_WATCH_GLOBAL] =
  runArmSubmitWatch;

// Ensure this file is treated as an ES module.
export {};
