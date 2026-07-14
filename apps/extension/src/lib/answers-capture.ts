/**
 * Answers-capture collector (runs in the page's isolated world).
 *
 * Injected via `capture.ts` (compiled to `capture.js`,
 * `chrome.scripting.executeScript({ files: ['capture.js'] })`) when the user
 * clicks "Save my answers from this page" in the popup. Walks FILLED, visible
 * `input[type=text]` / `textarea` / `select` elements, pairs each with its
 * label text, and returns `{question, answer}[]` for the background to send
 * on as `answers.save`.
 *
 * Shares the label/visibility/denylist primitives with `autofill.ts`
 * (`./field-signal`) so the two directions (fill vs. capture) never drift on
 * what counts as "labelled"/"hidden"/"ambiguous". This module and
 * `autofill.ts` are still bundled into SEPARATE injected files (`capture.js`
 * / `fill.js`) via isolated Rollup passes (see `vite.config.ts`'s
 * `injectedEntries` plugin), so the shared helpers are inlined into EACH
 * rather than hoisted into a chunk that either classic script would then
 * have to `import`.
 *
 * Pure DOM — no extension APIs, no network — so it is unit-testable against a
 * jsdom document.
 */

import { AMBIGUOUS, isHidden, labelText, matchNamedKey, textSignal } from './field-signal';

/** One captured question/answer pair. */
export interface CapturedAnswer {
  question: string;
  answer: string;
}

/** `<input>` types this collector reads. Narrower than autofill's
 *  `FILLABLE_TYPES` (no `email`/`tel`/`url`): a free-text application-form
 *  answer lives in a plain text input, and the omission avoids re-capturing
 *  the user's own contact details (already sourced from the profile) as an
 *  "answer". `''` is included defensively, same as autofill's set — a real
 *  `<input>` with no `type` attribute normalizes to `"text"`. */
const CAPTURABLE_INPUT_TYPES = new Set(['text', '']);

/** True when `el`'s free-text signal is visible, not on the ambiguous/
 *  sensitive denylist (the same discipline `autofill.ts`'s
 *  `isCandidateField` applies, minus autofill's fill-direction-only checks —
 *  autocomplete tokens, "already has a value"), AND not a known IDENTITY
 *  field (name/first/last/email/phone/linkedin/github/website/location, via
 *  the shared `matchNamedKey`) — a filled "Full Name" or "LinkedIn URL" text
 *  field is contact-profile data, not a genuine application question, and
 *  must never pollute `Application.answers`. */
function isCapturable(el: HTMLElement): boolean {
  if (isHidden(el)) return false;
  const signal = textSignal(el);
  if (AMBIGUOUS.some((w) => signal.includes(w))) return false;
  return matchNamedKey(signal) === null;
}

/** The selected option's VISIBLE text (not its `value` attribute) — per the
 *  capture spec, a `<select>`'s answer is what the user saw/chose, not the
 *  form's internal value. `''` when nothing is selected OR the selected
 *  option's `value` is empty — an empty value is how a placeholder option
 *  (e.g. `<option value="">Choose one</option>`) is conventionally marked as
 *  "nothing chosen" even though its visible text is non-blank; without this
 *  check that placeholder text would be captured as if it were a real
 *  answer. */
function selectAnswer(el: HTMLSelectElement): string {
  const opt = el.options.item(el.selectedIndex);
  if (!opt || opt.value === '') return '';
  return opt.text.trim();
}

/**
 * Scan `doc` for filled, visible, labelled `input[type=text]` / `textarea` /
 * `select` fields (skipping the ambiguous/sensitive denylist and any
 * unlabelled or blank-valued field) and return their question/answer pairs.
 * Pure — no side effects.
 */
export function collectAnswers(doc: Document): CapturedAnswer[] {
  const out: CapturedAnswer[] = [];
  const fields = doc.querySelectorAll<HTMLElement>('input, textarea, select');

  for (const el of Array.from(fields)) {
    let answer: string;
    if (el instanceof HTMLInputElement) {
      if (!CAPTURABLE_INPUT_TYPES.has(el.type)) continue;
      answer = el.value.trim();
    } else if (el instanceof HTMLTextAreaElement) {
      answer = el.value.trim();
    } else if (el instanceof HTMLSelectElement) {
      answer = selectAnswer(el);
    } else {
      continue;
    }
    if (!answer) continue;
    if (!isCapturable(el)) continue;

    const question = labelText(el).trim();
    if (!question) continue;

    out.push({ question, answer });
  }

  return out;
}
