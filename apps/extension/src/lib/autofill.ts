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
 *
 * The label/visibility/denylist primitives (`labelText`/`textSignal`/
 * `isHidden`/`escapeId`/`isAmbiguousSignal`) live in `./field-signal` — shared
 * with `answers-capture.ts` (PR 5 of the extension roadmap) so fill and capture
 * never disagree on what counts as labelled/visible/ambiguous.
 */

import {
  autocompleteToken,
  isAmbiguousSignal,
  isHidden,
  matchAutocompleteKey,
  matchNamedKey,
  textSignal,
} from './field-signal';

/**
 * Isolated-world global key under which `fill.ts` exposes {@link runAutofill};
 * the background's second `executeScript({ func })` reads it. Shared here so the
 * inject side and the call side can never disagree on the name.
 */
export const AUTOFILL_GLOBAL = '__ajhRunAutofill';

/** One additional labelled link beyond the named platform fields (e.g.
 *  Portfolio, Dribbble, Behance, Stack Overflow). Matched by Tier 2 only —
 *  see {@link matchExtraLink}. */
interface AutofillLink {
  label: string;
  url: string;
}

/** The flat contact-profile projection sent by the desktop for autofill. */
export interface AutofillProfile {
  fullName?: string;
  email?: string;
  phone?: string;
  location?: string;
  linkedin?: string;
  github?: string;
  website?: string;
  extraLinks?: AutofillLink[];
}

/** One profile field that was written, with how many form fields received it.
 *  An extra-link fill uses the synthetic key `extraLink:<label>` (its `label`
 *  is the link's own label, e.g. "Portfolio") so it never collides with a
 *  named key. */
interface AutofillFilledField {
  /** Logical key: email | phone | fullName | firstName | lastName | location | linkedin | github | website | extraLink:<label>. */
  key: string;
  /** Human label shown in the summary (e.g. "Email", or the extra link's own label). */
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
  /**
   * Count of otherwise-fillable fields skipped because their signal matched
   * MORE THAN ONE extra link — under-fill over guessing which one is right.
   * Optional (not just `0`) so every pre-existing `AutofillSummary` literal
   * (tests, the popup) stays valid without this field; `planAndFill` always
   * sets it.
   */
  skippedAmbiguous?: number;
}

/** DOM id of the injected summary overlay (also used to clear a prior pass). */
const OVERLAY_ID = 'ajh-autofill-overlay';

/** Input types we will consider filling. Everything else (password, hidden,
 *  number, date, search, checkbox, radio, file, submit, …) is skipped. */
const FILLABLE_TYPES = new Set(['text', 'email', 'tel', 'url', '']);

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

/** `AutofillFilledField.key` prefix for an extra-link fill (see {@link matchExtraLink}). */
const EXTRA_LINK_PREFIX = 'extraLink:';

/** Split a full name into first token + remainder (a flagged guess). */
export function splitName(fullName: string): { first: string; last: string } {
  const parts = fullName.trim().split(/\s+/).filter(Boolean);
  const first = parts[0] ?? '';
  const last = parts.slice(1).join(' ');
  return { first, last };
}

/**
 * The baseline gate shared by every fill tier: a fillable input TYPE, visible,
 * empty, not a card/password autocomplete token, and not matching the ambiguous/
 * sensitive denylist ({@link isAmbiguousSignal}). Extracted so the extra-link
 * matcher (Tier 2) applies the exact same discipline as the named-key matcher below.
 */
function isCandidateField(el: HTMLInputElement): boolean {
  if (!FILLABLE_TYPES.has(el.type)) return false;
  if (isHidden(el)) return false;
  if (el.value.trim() !== '') return false; // never overwrite

  const ac = autocompleteToken(el);
  if (ac.startsWith('cc-') || ac === 'current-password' || ac === 'new-password') return false;

  if (isAmbiguousSignal(textSignal(el))) return false;

  return true;
}

/**
 * Decide which logical profile key an input should receive, or `null` to skip.
 * Tier 1 = the standard `autocomplete` token (always fill). Tier 2 = an
 * unambiguous label/name/id/placeholder signal. Social/website under-fill: they
 * require a SPECIFIC signal (linkedin/github/portfolio/personal site); a bare
 * "Website"/"URL" is ambiguous and skipped.
 */
function matchFieldKey(el: HTMLInputElement): string | null {
  if (!isCandidateField(el)) return null;

  const signal = textSignal(el);

  // ── Tier 1: standard autocomplete tokens (see `matchAutocompleteKey`) ─────
  const tier1 = matchAutocompleteKey(autocompleteToken(el));
  if (tier1) return tier1;

  // ── Tier 2: unambiguous free-text signal (shared with answers-capture's
  // exclude-identity-fields check — see `matchNamedKey`) ────────────────────
  return matchNamedKey(signal);
}

/**
 * Passive "does this page have at least one autofill-supported identity
 * field?" probe — the SAME gates {@link planAndFill} uses ({@link matchFieldKey}
 * returning a matched key, which already requires {@link isCandidateField}),
 * but stops at the first match and never writes anything. Answers-capture's
 * own candidate lists deliberately EXCLUDE identity fields (they're
 * profile-sourced, not "application questions" — see `answers-capture.ts`'s
 * `isCapturable`), so a page with ONLY name/email/phone fields would
 * otherwise look answer-capture-empty even though "Fill this form" has
 * plenty to do. This is the other half of the popup's fields-probe union —
 * see `probe-fields.ts`.
 */
export function hasAutofillableFields(doc: Document): boolean {
  for (const el of Array.from(doc.querySelectorAll('input'))) {
    if (matchFieldKey(el) !== null) return true;
  }
  return false;
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
 * Extra-link labels too generic to ever safely match a field: a bare
 * "Website"/"Link" field either already resolves via the named `website` key
 * (Tier 1/2 above) or is genuinely ambiguous — it must never receive an
 * arbitrary secondary link, even in the edge case where the user themselves
 * labelled an extra link this generically.
 */
const GENERIC_LINK_LABELS = new Set([
  'website',
  'web site',
  'site',
  'link',
  'url',
  'web',
  'homepage',
  'home page',
  'personal site',
  'personal website',
  'profile',
  'portal',
]);

/**
 * {@link GENERIC_LINK_LABELS}, but run through {@link labelTokens}, sorted,
 * and re-joined with a single space — the same normalization the label/field
 * matcher itself uses below. `matchExtraLink` matching is TOKEN-based and
 * order-insensitive (case, diacritics, punctuation, and word order are all
 * ignored before comparison), so gating on the raw strings above — or on
 * tokens joined in their original order — would let a link labelled
 * "Website!"/"Web-Site"/"Site Web" slip past the denylist while still
 * token-matching a bare "Website" field. Comparing sorted-tokenized-vs-
 * sorted-tokenized keeps the two checks in lockstep regardless of word order.
 */
const GENERIC_LINK_LABEL_TOKENS = new Set(
  Array.from(GENERIC_LINK_LABELS, (label) => [...labelTokens(label)].sort().join(' '))
);

/** Lowercase, diacritic-stripped, trimmed normalization for label matching.
 *  NFD-decomposes (é → e + ´) then strips every combining mark via the
 *  `\p{Diacritic}` Unicode property escape. */
function normalizeLabel(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase()
    .trim();
}

/** A normalized label split into whole-word tokens (e.g. "Stack Overflow" → ["stack", "overflow"]). */
function labelTokens(label: string): string[] {
  return normalizeLabel(label)
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
}

/** Input types the extra-link matcher will fill — narrower than the baseline
 *  {@link FILLABLE_TYPES}: `email`/`tel` pass the named-key gate (a same-shaped
 *  value can go there), but a URL is syntactically invalid in either, so
 *  Tier-2 link matching never fires on them even if the label otherwise
 *  matches. */
const EXTRA_LINK_FILLABLE_TYPES = new Set(['text', 'url', '']);

/**
 * Tier-2 extra-link matcher: does this field's free-text signal contain EVERY
 * token of an extra link's label as a whole word (diacritic/case-insensitive)?
 * Conservative by construction — a partial/coincidental substring hit never
 * counts, and a link whose own label is one of {@link GENERIC_LINK_LABELS} is
 * excluded entirely. A field whose signal matches MORE THAN ONE distinct link
 * is ambiguous (`ambiguous: true`) — never guess which one is right.
 *
 * A single-token label (e.g. a link labelled just "Profile" or "Portal") is
 * the highest false-positive-risk case: a common English word is likely to
 * appear by coincidence in an unrelated field's name/id/placeholder/label.
 * {@link GENERIC_LINK_LABELS} is the mitigation — every label that is itself
 * a bare generic noun is excluded from matching entirely, no matter how the
 * rest of the form reads.
 */
function matchExtraLink(
  el: HTMLInputElement,
  links: readonly AutofillLink[]
): { link: AutofillLink | null; ambiguous: boolean } {
  if (!EXTRA_LINK_FILLABLE_TYPES.has(el.type)) return { link: null, ambiguous: false };

  // Normalize the signal the SAME way as the label (diacritic-strip first) —
  // otherwise a non-ASCII char in the field's own text would fracture a token
  // (e.g. "Stäck" splitting into "st"/"ck") before the comparison ever runs.
  const signalTokens = new Set(
    normalizeLabel(textSignal(el))
      .split(/[^a-z0-9]+/)
      .filter(Boolean)
  );
  const matches = links.filter((link) => {
    const tokens = labelTokens(link.label);
    const sortedTokenKey = [...tokens].sort().join(' ');
    if (tokens.length === 0 || GENERIC_LINK_LABEL_TOKENS.has(sortedTokenKey)) return false;
    return tokens.every((t) => signalTokens.has(t));
  });
  if (matches.length === 0) return { link: null, ambiguous: false };
  if (matches.length > 1) return { link: null, ambiguous: true };
  return { link: matches[0] ?? null, ambiguous: false };
}

/**
 * Scan `doc` for fillable inputs, fill each matching EMPTY field from `profile`,
 * and return a summary. Pure w.r.t. profile persistence — nothing is stored.
 *
 * A field a named key (Tier 1/2) matches WITH a value is filled from that key
 * exclusively — never additionally reconsidered against `extraLinks`. The ONE
 * exception is the `website` key: a field mapped to it but with NOTHING in
 * the profile (e.g. a "Portfolio" field maps to the generic `website` key,
 * but the profile's `website` is empty) falls through to the extra-link
 * matcher instead of being given up on — a specific "Portfolio" extra link is
 * a better answer than an empty guess. Every OTHER named key (email, phone,
 * linkedin, github, …) claims its field exclusively regardless of value — an
 * empty profile value there is a legitimate "nothing to fill", not a signal
 * to try the extra-link matcher, so those fields stay strictly additive.
 */
export function planAndFill(doc: Document, profile: AutofillProfile): AutofillSummary {
  const split = splitName(profile.fullName ?? '');
  const links = profile.extraLinks ?? [];
  const counts = new Map<string, number>();
  let usedSplit = false;
  let skippedAmbiguous = 0;

  for (const el of Array.from(doc.querySelectorAll('input'))) {
    if (!isCandidateField(el)) continue;

    const key = matchFieldKey(el);
    if (key) {
      const value = valueForKey(key, profile, split);
      if (value) {
        setValue(el, value);
        counts.set(key, (counts.get(key) ?? 0) + 1);
        if (key === 'firstName' || key === 'lastName') usedSplit = true;
        continue;
      }
      // Named slot matched but empty — only `website` falls through to the
      // extra-link matcher (the portfolio→website heuristic collision this
      // was built for); every other named key claims the field regardless of
      // value, so it stays skipped rather than reconsidered.
      if (key !== 'website') continue;
    }

    if (links.length === 0) continue;
    const { link, ambiguous } = matchExtraLink(el, links);
    if (ambiguous) {
      skippedAmbiguous += 1;
      continue;
    }
    if (!link) continue;
    setValue(el, link.url);
    const linkKey = `${EXTRA_LINK_PREFIX}${link.label}`;
    counts.set(linkKey, (counts.get(linkKey) ?? 0) + 1);
  }

  const filled: AutofillFilledField[] = Array.from(counts.entries()).map(([key, count]) => ({
    key,
    label: key.startsWith(EXTRA_LINK_PREFIX)
      ? key.slice(EXTRA_LINK_PREFIX.length)
      : (KEY_LABELS[key] ?? key),
    count,
  }));

  return {
    filled,
    nameSplit: usedSplit ? split : null,
    filledNothing: filled.length === 0,
    skippedAmbiguous,
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
    // Suppress this line when fields WERE skipped as ambiguous — the
    // skipped-note below already explains the outcome; showing both reads as
    // contradictory ("no matchable fields" + "N fields skipped").
    if (!summary.skippedAmbiguous) {
      const p = doc.createElement('div');
      p.textContent = 'No matchable fields found on this page. Nothing was changed.';
      box.appendChild(p);
    }
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

  if (summary.skippedAmbiguous) {
    const ambiguous = doc.createElement('div');
    ambiguous.style.cssText = 'margin-top:6px;color:#fbbf24';
    ambiguous.textContent = `${summary.skippedAmbiguous} field${summary.skippedAmbiguous === 1 ? '' : 's'} skipped — matched more than one saved link, so nothing was guessed.`;
    box.appendChild(ambiguous);
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
