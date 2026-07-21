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
 *  - UNDER-reports rather than over-reports. The message auto-advances a saved
 *    application to `applied`, so a false positive silently lies about what the
 *    user did; a false negative just leaves them to mark it by hand. Hence the
 *    {@link looksLikeApplicationForm} gate on the submit path and the strict
 *    text requirement for a control with no form around it.
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
 *  (an apply/submit/finish button), not a "save draft"/"add another" control.
 *  Only trusted for a control that sits inside an application form — see
 *  {@link STRICT_APPLY_TEXT_RE}. */
const APPLY_TEXT_RE = /apply|submit application|send application|finish/i;

/** The subset of {@link APPLY_TEXT_RE} trusted with NO surrounding form to
 *  corroborate it (a pure SPA control). A bare "Apply"/"Apply now" is excluded:
 *  outside a form it is overwhelmingly the button that OPENS the application,
 *  not the one that sends it. */
const STRICT_APPLY_TEXT_RE =
  /submit\s+(?:your|my|the)?\s*application|send\s+(?:your|my|the)?\s*application|finish/i;

/** Minimum number of visible, fillable fields for a `<form>` to be treated as an
 *  application form. A site search box, a newsletter signup and a login form all
 *  have one or two; a real application form has more (and usually a résumé file
 *  input, which short-circuits this check outright). */
const MIN_APPLICATION_FIELDS = 3;

/**
 * Whether `form` looks like the application form rather than incidental page
 * furniture (search box, filter, newsletter signup, login).
 *
 * The `submit` event fires for EVERY form on the page, and the watcher has no
 * other way to tell them apart — it only reports `location.href`, so a search
 * submit was indistinguishable from sending the application.
 */
function looksLikeApplicationForm(form: HTMLFormElement): boolean {
  const fields = Array.from(form.querySelectorAll('input, textarea, select')).filter(
    (el): el is HTMLElement => el instanceof HTMLElement && !isHidden(el)
  );
  const fillable = fields.filter((el) => {
    const type = (el.getAttribute('type') ?? '').toLowerCase();
    return !['hidden', 'submit', 'button', 'image', 'reset', 'search'].includes(type);
  });
  // A file input is the résumé upload — decisive on its own.
  if (fillable.some((el) => (el.getAttribute('type') ?? '').toLowerCase() === 'file')) return true;
  return fillable.length >= MIN_APPLICATION_FIELDS;
}

/** The application form this control belongs to, if any — `form` for a native
 *  submit control, otherwise the nearest `<form>` ancestor (SPA `role=button`). */
function applicationFormFor(el: HTMLElement): HTMLFormElement | null {
  const form = (el as HTMLButtonElement | HTMLInputElement).form ?? el.closest?.('form') ?? null;
  return form instanceof HTMLFormElement ? form : null;
}

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
  // Inside an application form the surrounding structure corroborates the text,
  // so the broad pattern is trusted. With no form to corroborate it, only an
  // explicit send verb counts — a bare "Apply now" out there is the button that
  // OPENS the application.
  const form = el instanceof HTMLElement ? applicationFormFor(el) : null;
  if (form) return looksLikeApplicationForm(form) && APPLY_TEXT_RE.test(text);
  return STRICT_APPLY_TEXT_RE.test(text);
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

  // Real form submit — but ONLY for a form that looks like the application form.
  // The listener sees every form on the page and reports nothing but
  // `location.href`, so without this check a search box, filter or newsletter
  // signup submit was indistinguishable from sending the application.
  doc.addEventListener(
    'submit',
    (ev) => {
      const form = ev.target;
      if (!(form instanceof HTMLFormElement) || !looksLikeApplicationForm(form)) return;
      fire();
    },
    true
  );

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
