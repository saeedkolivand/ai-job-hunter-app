/**
 * Gesture-armed form-submit watcher (Task #22, auto-track Layer A) — pure DOM.
 *
 * After the user has invoked the extension on an application page (an existing
 * injection gesture), the background injects `submit-watch.js`, which calls
 * {@link armSubmitWatch} to listen — capture-phase, OBSERVE-ONLY — for either a
 * real form submit or a click on an apply-style control, and posts the page URL
 * back to the background exactly once. The background then decides whether the
 * matched application should be auto-marked `applied` (see `./auto-track`).
 *
 * Invariants:
 *  - NEVER blocks or alters the submit: no `preventDefault`/`stopPropagation`,
 *    so the real form submission proceeds untouched.
 *  - Fires AT MOST ONCE per arming — a click on the apply button AND the submit
 *    it triggers post a single message, not two (the `fired` closure guard).
 *  - Reads `location.href` SYNCHRONOUSLY in the handler, so a full-page-nav
 *    submit still delivers the current URL before the document unloads.
 *
 * Pure DOM — no extension APIs — so it is unit-testable against a jsdom
 * document (the classic-script entry `submit-watch.ts` supplies the real
 * `chrome.runtime.sendMessage` post). Visibility is computed-style-ONLY
 * (reusing {@link isHidden} from `./field-signal`) — never
 * `getBoundingClientRect`/`offsetWidth`, which jsdom always reports as zero.
 *
 * Build note: like `fill.ts`/`answer-fill.ts`, `submit-watch.js` is injected
 * via `chrome.scripting.executeScript({ files: [...] })` as a CLASSIC script, so
 * it must carry ZERO `import` statements after the isolated Rollup pass
 * (`vite.config.ts`'s `injectedEntries`) inlines this module + its `field-signal`
 * dependency.
 */

import { isHidden } from './field-signal';

/** Internal background message kind the injected watcher posts on a detected
 *  submit. Duplicated as a plain literal in `background.ts` (kept out of that
 *  bundle's import graph — same discipline as `AUTOFILL_GLOBAL`). */
export const SUBMIT_DETECTED_MSG = 'submitDetected';

/** Visible text that marks a control as a real "send the application" action
 *  (an apply/submit/finish button), not a "save draft"/"add another" control. */
const APPLY_TEXT_RE = /apply|submit application|send application|finish/i;

/**
 * True when `el` is an apply-style control worth treating as a submit: a real
 * submit button/input, OR an ARIA `role="button"` (Easy-Apply / SPA flows that
 * submit via JS and never fire a native `submit` event), whose VISIBLE text
 * matches {@link APPLY_TEXT_RE}. Visibility is checked by the CALLER (so the
 * pure text/role predicate stays trivially testable).
 */
function isApplyControl(el: Element): boolean {
  const tag = el.tagName;
  const type = (el.getAttribute('type') ?? '').toLowerCase();
  const isSubmitButton = tag === 'BUTTON' && type === 'submit';
  const isSubmitInput = tag === 'INPUT' && type === 'submit';
  const isRoleButton = el.getAttribute('role') === 'button';
  if (!isSubmitButton && !isSubmitInput && !isRoleButton) return false;
  const text =
    el instanceof HTMLInputElement
      ? el.value
      : `${el.textContent ?? ''} ${el.getAttribute('aria-label') ?? ''}`;
  return APPLY_TEXT_RE.test(text);
}

/**
 * Arm the observe-only watcher on `doc`: a capture-phase `submit` listener (any
 * real form submit) PLUS a capture-phase click heuristic for apply-style
 * controls (for Easy-Apply/SPA flows that submit via JS without a native submit
 * event). The FIRST of either posts the page URL via `post` exactly once; every
 * later event is a no-op. Never blocks or alters the observed event.
 */
export function armSubmitWatch(doc: Document, post: (url: string) => void): void {
  let fired = false;
  const fire = (): void => {
    if (fired) return;
    fired = true;
    post(doc.defaultView?.location?.href ?? '');
  };

  // Real form submit — any submit path that fires the native event.
  doc.addEventListener('submit', () => fire(), true);

  // Apply-style click — backstop for role=button / JS-driven submits that never
  // fire a native `submit`. The click can land on a child (an icon/span), so
  // walk up to the nearest candidate control and only fire for a VISIBLE one
  // (skip an off-screen/honeypot button — computed-style-only via `isHidden`).
  doc.addEventListener(
    'click',
    (ev) => {
      const start = ev.target;
      if (!(start instanceof Element)) return;
      const el = start.closest('button, input, [role="button"]');
      if (el instanceof HTMLElement && isApplyControl(el) && !isHidden(el)) fire();
    },
    true
  );
}
