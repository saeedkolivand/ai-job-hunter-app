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
 * arms the DOM LISTENERS at most once per frame via an isolated-world global
 * flag (same discipline as `AUTOFILL_GLOBAL`). The `captureAnswers` VALUE is
 * NOT frozen at that first arm, though (PR-1209 finding) — it is stored in a
 * mutable module-level variable updated on EVERY call, and the watcher reads
 * it fresh at fire time (a getter, not a boolean, passed to
 * `armSubmitWatch`), so a later re-arm with a DIFFERENT `captureAnswers`
 * value (the desktop-enforced opt-in toggled mid-frame) takes effect on this
 * SAME arming without needing a navigation. The one extension API this file
 * touches is `chrome.runtime.sendMessage` (available in the injected
 * isolated world), used to post the detected URL (plus any captured
 * answers) back to the background — fire-and-forget.
 */

import type { CapturedAnswer } from './lib/answers-capture';
import { armSubmitWatch, SUBMIT_DETECTED_MSG, SUBMIT_WATCH_GLOBAL } from './lib/submit-watch';

/** Isolated-world arm-once flag: re-injection after another gesture on the same
 *  frame must not stack a second listener set. */
const ARMED_FLAG = '__ajhSubmitWatchArmed';

/** Isolated-world MUTABLE consent value, updated on EVERY `runArmSubmitWatch`
 *  call (not gated by {@link ARMED_FLAG}) — the desktop-enforced
 *  `saveAnswersOnSubmit` opt-in can change mid-frame (background.ts's
 *  `maybeArmSubmitWatch` re-reads it fresh on every subsequent gesture and
 *  re-injects), so freezing the FIRST value for the DOM-listener's whole life
 *  meant turning the switch off still captured on a later submit, and
 *  turning it on did nothing until a navigation (PR-1209 finding). The
 *  listeners themselves are still armed AT MOST ONCE per frame; only this
 *  value is live. Kept policy-free here too — this file only stores the
 *  value the background handed it, never decides it. */
let currentCaptureAnswers = false;

// `chrome` is available in the injected isolated-world content-script context in
// both Chrome and Firefox; declared locally so this classic script pulls in no
// extension-types dependency.
declare const chrome: { runtime: { sendMessage(message: unknown): unknown } };

function runArmSubmitWatch(captureAnswers: boolean): void {
  currentCaptureAnswers = captureAnswers;
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
    // A getter, not the snapshot `captureAnswers` argument — read FRESH at
    // fire time so a later re-arm's updated value takes effect immediately.
    { captureAnswers: () => currentCaptureAnswers }
  );
}

(globalThis as unknown as Record<string, (captureAnswers: boolean) => void>)[SUBMIT_WATCH_GLOBAL] =
  runArmSubmitWatch;

// Ensure this file is treated as an ES module.
export {};
