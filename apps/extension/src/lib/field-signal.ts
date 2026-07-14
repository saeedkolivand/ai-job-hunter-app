/**
 * Shared form-field signal primitives — label text, visibility, and the
 * ambiguous/sensitive denylist. Factored out of `autofill.ts` (PR 5 of the
 * extension roadmap) so BOTH `fill.ts` (assisted autofill, via
 * `autofill.ts`) and `capture.ts` (answers capture, via
 * `answers-capture.ts`) share ONE definition of "what counts as a labelled /
 * visible / ambiguous field" — never two copies that could drift.
 *
 * Pure DOM — no extension APIs, no network — so it is unit-testable against a
 * jsdom document. Behavior is UNCHANGED from what previously lived inline in
 * `autofill.ts`; this is a pure extraction (its existing tests pin the
 * behavior and pass unmodified).
 *
 * Build note: `fill.js` and `capture.js` are each injected via
 * `chrome.scripting.executeScript({ files: [...] })` as CLASSIC scripts (no
 * ES module support) — they must bundle with ZERO `import` statements. Since
 * BOTH now genuinely share this module at runtime, `vite.config.ts` builds
 * each in its OWN isolated Rollup pass (the `injectedEntries` plugin) so this
 * file is inlined into EACH bundle rather than hoisted into a shared chunk
 * that either classic script would then have to `import`.
 */

/**
 * Substrings that make a field ambiguous or sensitive — a match on the label /
 * name / id / placeholder skips the field entirely (under-fill over mis-fill
 * for autofill; skip-don't-capture for answers capture). Includes the grilled
 * set (referrer/emergency/confirm/manager/parent) plus the fields most likely
 * to receive the WRONG identity on a job-application form
 * (company/employer/recruiter), login/search noise, and — defense-in-depth —
 * sensitive PII categories that should never be touched even by accident
 * (SSN/tax id, passport, date of birth).
 */
export const AMBIGUOUS = [
  'referr',
  'referral',
  'reference',
  'emergency',
  'confirm',
  'manager',
  'supervisor',
  'parent',
  'guardian',
  'company',
  'employer',
  'organization',
  'organisation',
  'recruiter',
  'search',
  'username',
  'user name',
  'password',
  'captcha',
  'coupon',
  'promo',
  'maiden',
  'ssn',
  'social security',
  'tax',
  'passport',
  'dob',
  'birth',
  'date of birth',
];

/**
 * True when `el` or ANY ancestor is hidden — via the `hidden` attribute or
 * COMPUTED style (not just inline `style`): `display:none`/`visibility:hidden`,
 * `opacity:0`, off-screen absolute/fixed positioning (`left`/`top` shoved past
 * -9999px — the classic honeypot trap), or a box whose computed `width` AND
 * `height` are BOTH exactly `0px`. Computed style (not just inline `style`) is
 * what catches an external-stylesheet / `<style>` CSS-class honeypot — this is
 * how anti-bot honeypot fields are commonly planted on real ATS forms
 * (Greenhouse/Lever/Workday). An inline-only or display/visibility-only check
 * would fill/capture them, and a filled invisible field is worse than an
 * ordinary mis-fill (the user can't see it to undo, and it can flag them as a
 * bot).
 *
 * NOT caught, deliberately: clip-based hiding (`clip:rect(0,0,0,0)`/
 * `clip-path`) or a single-dimension-zero box (e.g. the `width:1px;height:1px`
 * shape common to `.sr-only`-style utility classes) — that is also exactly how a
 * LEGITIMATE screen-reader-only field is hidden visually while staying
 * functionally real, so treating it as hidden (and skipping it) would be
 * wrong. Only an unambiguous honeypot shape — display/visibility/opacity-off,
 * off-screen, or BOTH dimensions zero — is treated as hidden.
 *
 * Deliberately `getComputedStyle`-ONLY — never `getBoundingClientRect`/
 * `offsetWidth`/layout reads. jsdom (the test environment) always reports those
 * as zero, which would make every field — including normal visible ones — read
 * as hidden. Computed style has no such gap: a real field's computed `width` is
 * `auto`/a real length (never the literal string `'0px'`), its `position` is
 * `static`, and its `opacity` is `1`, so this stays jsdom-safe.
 */
export function isHidden(el: HTMLElement): boolean {
  const view = el.ownerDocument.defaultView;
  let node: HTMLElement | null = el;
  while (node) {
    if (node.hidden) return true;
    const cs = view?.getComputedStyle(node);
    if (cs) {
      if (cs.display === 'none' || cs.visibility === 'hidden') return true;
      if (Number.parseFloat(cs.opacity) === 0) return true;
      if (
        (cs.position === 'absolute' || cs.position === 'fixed') &&
        (Number.parseFloat(cs.left) <= -9999 || Number.parseFloat(cs.top) <= -9999)
      )
        return true;
      if (cs.width === '0px' && cs.height === '0px') return true;
    }
    node = node.parentElement;
  }
  return false;
}

/** CSS.escape when available (jsdom + browsers), else a conservative fallback. */
export function escapeId(id: string): string {
  if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(id);
  return id.replace(/["\\]/g, '\\$&');
}

/** The associated label text for a form element: `<label for>` + any wrapping
 *  `<label>`. Takes `HTMLElement` (not just `HTMLInputElement`) so it works
 *  identically for `<textarea>`/`<select>` — every member it touches
 *  (`id`/`closest`) is generic to `Element`, not input-specific. */
export function labelText(el: HTMLElement): string {
  const doc = el.ownerDocument;
  let text = '';
  if (el.id) {
    const forLabel = doc.querySelector(`label[for="${escapeId(el.id)}"]`);
    if (forLabel?.textContent) text += ` ${forLabel.textContent}`;
  }
  const wrapping = el.closest('label');
  if (wrapping?.textContent) text += ` ${wrapping.textContent}`;
  return text;
}

/** The lowercased free-text signal (name/id/placeholder/aria-label/label) used
 *  both for autofill's Tier-2 field matching and the answers-capture
 *  denylist check. Takes `HTMLElement` for the same reason as `labelText`. */
export function textSignal(el: HTMLElement): string {
  return [
    el.getAttribute('name') ?? '',
    el.id,
    el.getAttribute('placeholder') ?? '',
    el.getAttribute('aria-label') ?? '',
    labelText(el),
  ]
    .join(' ')
    .toLowerCase();
}

/** The last (field) token of an `autocomplete` attribute value, e.g.
 *  "shipping email" → "email". `''` for a missing/`off`/`on` attribute. Takes
 *  `HTMLElement` (not just `HTMLInputElement`) so `answers-capture.ts` can call
 *  it on a `<textarea>`/`<select>` too — an `autocomplete` attribute there
 *  simply normalizes to `''`/`off`, same as no match. Shared so autofill's
 *  Tier-1 token reading and answers-capture's identity check never drift on
 *  what the raw token is. */
export function autocompleteToken(el: HTMLElement): string {
  const raw = (el.getAttribute('autocomplete') ?? '').trim().toLowerCase();
  if (!raw || raw === 'off' || raw === 'on') return '';
  return raw.split(/\s+/).at(-1) ?? '';
}

/**
 * Map a standard `autocomplete` {@link autocompleteToken} to autofill's Tier-1
 * logical key, or `null` for a token with no fill/identity meaning here.
 * Mirrors `matchFieldKey`'s Tier-1 switch (`autofill.ts`) — factored out here
 * so `isCapturable` (`answers-capture.ts`) can exclude a field whose
 * `autocomplete` attribute marks it as identity (e.g. `autocomplete="name"`)
 * WITHOUT duplicating the token→key literals in a second copy.
 */
export function matchAutocompleteKey(token: string): string | null {
  switch (token) {
    case 'email':
      return 'email';
    case 'tel':
    case 'tel-national':
    case 'tel-local':
      return 'phone';
    case 'given-name':
      return 'firstName';
    case 'family-name':
      return 'lastName';
    case 'name':
      return 'fullName';
    case 'url':
      return 'website';
    // Only the city-level address token maps to the single free-text location;
    // street/postal/state/country sub-parts can't be filled from one string.
    case 'address-level2':
      return 'location';
    default:
      return null;
  }
}

/**
 * Resolve a lowercased field {@link textSignal} to a known identity key, or
 * `null` when it doesn't unambiguously match one. This is autofill's "Tier 2"
 * signal matching (`matchFieldKey` in `autofill.ts`), factored out so it's
 * shared with answers-capture: `isCapturable` (`answers-capture.ts`) calls it
 * to EXCLUDE any field whose signal identifies it as one of these keys — a
 * filled "Full Name" or "LinkedIn URL" text field must never be captured into
 * `Application.answers` as if it were a genuine application question. Pure
 * string matching — no element/autocomplete-attribute lookup (that stays
 * `autofill.ts`-only "Tier 1", since capture also runs against `<select>`/
 * `<textarea>` which don't carry the same autocomplete semantics).
 */
export function matchNamedKey(signal: string): string | null {
  if (signal.includes('linkedin')) return 'linkedin';
  if (signal.includes('github')) return 'github';
  if (signal.includes('portfolio') || /personal (web ?site|site)/.test(signal)) return 'website';
  if (signal.includes('email') || signal.includes('e-mail')) return 'email';
  if (signal.includes('phone') || signal.includes('mobile') || signal.includes('telephone'))
    return 'phone';
  if (/first name|given name|forename/.test(signal)) return 'firstName';
  if (/last name|surname|family name/.test(signal)) return 'lastName';
  if (signal.includes('city') || signal.includes('town') || /\blocation\b/.test(signal))
    return 'location';
  if (/\bfull name\b/.test(signal)) return 'fullName';
  // A bare "Name" field (not user/file/nick/display/business/org and not an
  // education field — "School Name"/"University Name"/"Degree Name"/… — and not
  // a first/last variant already handled) → full name.
  if (
    /\bname\b/.test(signal) &&
    !/user|file|nick|screen|display|business|org|school|institution|university|college|degree|course|program|certificat/.test(
      signal
    )
  )
    return 'fullName';

  return null;
}
