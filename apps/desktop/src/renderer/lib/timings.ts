/**
 * Shared UI timing constants (milliseconds).
 *
 * Only values used in two or more places (or representing a clear shared UX
 * pattern) are defined here. Genuinely one-off delays stay inline at the call
 * site (see "left as one-offs" below).
 *
 *   COPY_FEEDBACK_MS      1 500 ms — clipboard copy icon reset (short copy actions)
 *   COPY_FEEDBACK_LONG_MS 1 800 ms — clipboard copy icon reset (long-form content)
 *   TOOLTIP_HIDE_MS       2 000 ms — tooltip / success-flash auto-dismiss
 *
 * Left as one-offs:
 *   1 400 ms  ExtensionBridgeSection highlight-fade   — single use, semantically specific
 *   3 500 ms  ApplicationsPage row highlight flash    — single use
 *   1 200 ms  AISelectionStep skip transition delay   — single use
 *   1 000 ms  JobsPage stream-invalidate throttle     — single use, internal throttle
 *   350 ms    BuilderWizard RHF→Zustand sync debounce — single use, internal
 */
export const COPY_FEEDBACK_MS = 1_500;
export const COPY_FEEDBACK_LONG_MS = 1_800;
export const TOOLTIP_HIDE_MS = 2_000;
