/**
 * Assisted-autofill matcher + filler (runs in the page's isolated world).
 *
 * Injected on the user's click via `chrome.scripting.executeScript` (see
 * `src/fill.ts`, the thin entry that exposes {@link runAutofill} on the page
 * global; the background then calls it with the contact profile fetched fresh
 * from the desktop). This module is pure DOM — no extension APIs, no network — so
 * it is unit-testable against a jsdom document and safe to serialize into a page.
 *
 * Design guarantees (do not regress):
 *  - Fills only EMPTY, visible, fillable fields; never overwrites user input.
 *  - Under-fills rather than mis-fills: ambiguous / sensitive fields are skipped.
 *  - Never auto-submits; only sets values + dispatches input/change events.
 *  - Renders an in-page summary of exactly what it did (incl. the "nothing
 *    matched" case) so a no-op never looks broken. Name is split (first token /
 *    remainder) for separate first/last fields and FLAGGED as a guess.
 */

/**
 * Isolated-world global key under which `fill.ts` exposes {@link runAutofill};
 * the background's second `executeScript({ func })` reads it. Shared here so the
 * inject side and the call side can never disagree on the name.
 */
export const AUTOFILL_GLOBAL = '__ajhRunAutofill';

/** The flat contact-profile projection sent by the desktop for autofill. */
export interface AutofillProfile {
  fullName?: string;
  email?: string;
  phone?: string;
  location?: string;
  linkedin?: string;
  github?: string;
  website?: string;
}

/** One profile field that was written, with how many form fields received it. */
export interface AutofillFilledField {
  /** Logical key: email | phone | fullName | firstName | lastName | location | linkedin | github | website. */
  key: string;
  /** Human label shown in the summary (e.g. "Email"). */
  label: string;
  /** How many form fields received this value. */
  count: number;
}

/** The result of a fill pass — drives both the in-page overlay and the popup. */
export interface AutofillSummary {
  filled: AutofillFilledField[];
  /**
   * Present when a separate first/last field was filled from splitting the full
   * name — flagged so the user verifies the (heuristic) split.
   */
  nameSplit: { first: string; last: string } | null;
  /** True when no field matched — surfaced so a no-op reads as intentional. */
  filledNothing: boolean;
}

/** DOM id of the injected summary overlay (also used to clear a prior pass). */
const OVERLAY_ID = 'ajh-autofill-overlay';

/** Input types we will consider filling. Everything else (password, hidden,
 *  number, date, search, checkbox, radio, file, submit, …) is skipped. */
const FILLABLE_TYPES = new Set(['text', 'email', 'tel', 'url', '']);

/**
 * Substrings that make a field ambiguous or sensitive — a match on the label /
 * name / id / placeholder skips the field entirely (under-fill over mis-fill).
 * Includes the grilled set (referrer/emergency/confirm/manager/parent) plus the
 * fields most likely to receive the WRONG identity on a job-application form
 * (company/employer/recruiter), login/search noise, and — defense-in-depth, since
 * the matcher only ever writes our own name/email/phone/socials — sensitive PII
 * categories it should never touch even by accident (SSN/tax id, passport,
 * date of birth).
 */
const AMBIGUOUS = [
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

/** Human labels for the summary, per logical key. */
const KEY_LABELS: Record<string, string> = {
  email: 'Email',
  phone: 'Phone',
  fullName: 'Name',
  firstName: 'First name',
  lastName: 'Last name',
  location: 'Location',
  linkedin: 'LinkedIn',
  github: 'GitHub',
  website: 'Website',
};

/** Split a full name into first token + remainder (a flagged guess). */
export function splitName(fullName: string): { first: string; last: string } {
  const parts = fullName.trim().split(/\s+/).filter(Boolean);
  const first = parts[0] ?? '';
  const last = parts.slice(1).join(' ');
  return { first, last };
}

/**
 * True when the element or ANY ancestor is hidden — via the `hidden` attribute
 * or COMPUTED style (not just inline `style`): `display:none`/`visibility:hidden`,
 * `opacity:0`, off-screen absolute/fixed positioning (`left`/`top` shoved past
 * -9999px — the classic honeypot trap), or a box whose computed `width` AND
 * `height` are BOTH exactly `0px`. Computed style (not just inline `style`) is
 * what catches an external-stylesheet / `<style>` CSS-class honeypot — this is
 * how anti-bot honeypot fields are commonly planted on real ATS forms
 * (Greenhouse/Lever/Workday). An inline-only or display/visibility-only check
 * would fill them, and a filled invisible field is worse than an ordinary
 * mis-fill (the user can't see it to undo, and it can flag them as a bot).
 *
 * NOT caught, deliberately: clip-based hiding (`clip:rect(0,0,0,0)`/
 * `clip-path`) or a single-dimension-zero box (e.g. the `width:1px;height:1px`
 * shape common to `.sr-only`-style utility classes) — that is also exactly how a
 * LEGITIMATE screen-reader-only field is hidden visually while staying
 * functionally real, so treating it as hidden (and skip-filling it) would be
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
function isHidden(el: HTMLElement): boolean {
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
function escapeId(id: string): string {
  if (typeof CSS !== 'undefined' && typeof CSS.escape === 'function') return CSS.escape(id);
  return id.replace(/["\\]/g, '\\$&');
}

/** The associated label text for an input: `<label for>` + any wrapping `<label>`. */
function labelText(el: HTMLInputElement): string {
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

/** The lowercased free-text signal for Tier-2 matching (name/id/placeholder/aria/label). */
function textSignal(el: HTMLInputElement): string {
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

/** The last (field) token of an `autocomplete` value, e.g. "shipping email" → "email". */
function autocompleteField(el: HTMLInputElement): string {
  const raw = (el.getAttribute('autocomplete') ?? '').trim().toLowerCase();
  if (!raw || raw === 'off' || raw === 'on') return '';
  return raw.split(/\s+/).at(-1) ?? '';
}

/**
 * Decide which logical profile key an input should receive, or `null` to skip.
 * Tier 1 = the standard `autocomplete` token (always fill). Tier 2 = an
 * unambiguous label/name/id/placeholder signal. Social/website under-fill: they
 * require a SPECIFIC signal (linkedin/github/portfolio/personal site); a bare
 * "Website"/"URL" is ambiguous and skipped.
 */
export function matchFieldKey(el: HTMLInputElement): string | null {
  const type = el.type;
  if (!FILLABLE_TYPES.has(type)) return null;
  if (isHidden(el)) return null;
  if (el.value.trim() !== '') return null; // never overwrite

  const ac = autocompleteField(el);
  if (ac.startsWith('cc-') || ac === 'current-password' || ac === 'new-password') return null;

  const signal = textSignal(el);
  if (AMBIGUOUS.some((w) => signal.includes(w))) return null;

  // ── Tier 1: standard autocomplete tokens ──────────────────────────────────
  switch (ac) {
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
      break;
  }

  // ── Tier 2: unambiguous free-text signal ──────────────────────────────────
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

/** The value for a logical key from the profile (+ split for first/last). */
function valueForKey(
  key: string,
  profile: AutofillProfile,
  split: { first: string; last: string }
): string {
  switch (key) {
    case 'email':
      return profile.email ?? '';
    case 'phone':
      return profile.phone ?? '';
    case 'fullName':
      return profile.fullName ?? '';
    case 'firstName':
      return split.first;
    case 'lastName':
      return split.last;
    case 'location':
      return profile.location ?? '';
    case 'linkedin':
      return profile.linkedin ?? '';
    case 'github':
      return profile.github ?? '';
    case 'website':
      return profile.website ?? '';
    default:
      return '';
  }
}

/** Set a value the way a framework-controlled input notices (native setter + events). */
function setValue(el: HTMLInputElement, value: string): void {
  const proto = Object.getPrototypeOf(el) as object;
  const desc = Object.getOwnPropertyDescriptor(proto, 'value');
  if (desc?.set) desc.set.call(el, value);
  else el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
}

/**
 * Scan `doc` for fillable inputs, fill each matching EMPTY field from `profile`,
 * and return a summary. Pure w.r.t. profile persistence — nothing is stored.
 */
export function planAndFill(doc: Document, profile: AutofillProfile): AutofillSummary {
  const split = splitName(profile.fullName ?? '');
  const counts = new Map<string, number>();
  let usedSplit = false;

  for (const el of Array.from(doc.querySelectorAll('input'))) {
    const key = matchFieldKey(el);
    if (!key) continue;
    const value = valueForKey(key, profile, split);
    if (!value) continue; // profile has nothing for this field → leave it empty
    setValue(el, value);
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (key === 'firstName' || key === 'lastName') usedSplit = true;
  }

  const filled: AutofillFilledField[] = Array.from(counts.entries()).map(([key, count]) => ({
    key,
    label: KEY_LABELS[key] ?? key,
    count,
  }));

  return {
    filled,
    nameSplit: usedSplit ? split : null,
    filledNothing: filled.length === 0,
  };
}

/**
 * Inject a small, dismissable summary overlay into `doc`. Replaces any overlay
 * from a previous pass. Inline-styled (isolated world, no external CSS) with a
 * high z-index; the close button removes it.
 */
export function renderSummaryOverlay(doc: Document, summary: AutofillSummary): void {
  doc.getElementById(OVERLAY_ID)?.remove();

  const box = doc.createElement('div');
  box.id = OVERLAY_ID;
  box.setAttribute('role', 'status');
  box.style.cssText = [
    'position:fixed',
    'z-index:2147483647',
    'right:16px',
    'bottom:16px',
    'max-width:320px',
    'padding:12px 14px',
    'border-radius:10px',
    'background:#0f172a',
    'color:#e2e8f0',
    'font:13px/1.4 system-ui,-apple-system,sans-serif',
    'box-shadow:0 8px 24px rgba(0,0,0,.35)',
  ].join(';');

  const title = doc.createElement('div');
  title.textContent = 'AI Job Hunter — autofill';
  title.style.cssText = 'font-weight:600;margin-bottom:6px';
  box.appendChild(title);

  if (summary.filledNothing) {
    const p = doc.createElement('div');
    p.textContent = 'No matchable fields found on this page. Nothing was changed.';
    box.appendChild(p);
  } else {
    const list = doc.createElement('div');
    for (const f of summary.filled) {
      const row = doc.createElement('div');
      row.textContent = `${f.label} → ${f.count} field${f.count === 1 ? '' : 's'}`;
      list.appendChild(row);
    }
    if (summary.nameSplit) {
      const note = doc.createElement('div');
      note.style.cssText = 'margin-top:6px;color:#fbbf24';
      note.textContent = `Name split (guess) — First: ${summary.nameSplit.first} / Last: ${summary.nameSplit.last} — verify`;
      list.appendChild(note);
    }
    const verify = doc.createElement('div');
    verify.style.cssText = 'margin-top:6px;color:#94a3b8';
    verify.textContent = 'Review the filled fields, then submit yourself.';
    list.appendChild(verify);
    box.appendChild(list);
  }

  const close = doc.createElement('button');
  close.type = 'button';
  close.textContent = 'Dismiss';
  close.style.cssText =
    'margin-top:10px;padding:4px 10px;border:0;border-radius:6px;background:#334155;color:#e2e8f0;cursor:pointer;font:inherit';
  close.addEventListener('click', () => box.remove());
  box.appendChild(close);

  (doc.body ?? doc.documentElement).appendChild(box);
}

/**
 * The injected entry-point: fill the current document from `profile`, render the
 * summary overlay, and return the summary (for the popup). Kept side-effect-first
 * so `chrome.scripting.executeScript` gets a serializable return value.
 */
export function runAutofill(profile: AutofillProfile): AutofillSummary {
  const summary = planAndFill(document, profile);
  renderSummaryOverlay(document, summary);
  return summary;
}
