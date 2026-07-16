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

import {
  AMBIGUOUS,
  autocompleteToken,
  isHidden,
  labelText,
  matchAutocompleteKey,
  matchNamedKey,
  textSignal,
} from './field-signal';

/** One captured question/answer pair. */
export interface CapturedAnswer {
  question: string;
  answer: string;
}

/**
 * One scanned EMPTY candidate field for "questions mode" (`answers.suggest`'s
 * collector — {@link collectQuestions} below). `index` is this field's
 * OCCURRENCE among every candidate sharing the EXACT SAME `question` text
 * (0-based) — the stable fill-target correlation: {@link locateQuestionField}
 * re-scans with the IDENTICAL predicate and picks the field at
 * `(question, index)`, so a page mutation between scan and fill (a different
 * count/order of same-labelled fields) fails safe (no match at that index)
 * instead of ever filling a DIFFERENT field than the one that was scanned.
 */
export interface ScannedQuestion {
  question: string;
  index: number;
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
 *  "already has a value"), AND not a known IDENTITY field
 *  (name/first/last/email/phone/linkedin/github/website/location) — via
 *  EITHER the shared `matchNamedKey` free-text signal OR the field's own
 *  `autocomplete` attribute mapped through autofill's Tier-1
 *  `matchAutocompleteKey` — a filled "Full Name" text field, or one merely
 *  marked `autocomplete="name"` under a quirky label, is contact-profile
 *  data, not a genuine application question, and must never pollute
 *  `Application.answers`. */
function isCapturable(el: HTMLElement): boolean {
  if (isHidden(el)) return false;
  if (matchAutocompleteKey(autocompleteToken(el)) !== null) return false;
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

/** `true` when `el`'s current value counts as "empty" for questions-mode
 *  (the mirror check of `collectAnswers`'s "has a value" gate): a blank
 *  text/textarea, or a `<select>` with nothing meaningfully chosen (see
 *  {@link selectAnswer}). */
function isEmptyValue(el: HTMLElement): boolean {
  if (el instanceof HTMLInputElement) return el.value.trim() === '';
  if (el instanceof HTMLTextAreaElement) return el.value.trim() === '';
  if (el instanceof HTMLSelectElement) return selectAnswer(el) === '';
  return false;
}

/**
 * Every EMPTY, visible, labelled, non-ambiguous/non-identity candidate field
 * — `input[type=text]` / `textarea` / `select`, same gates as
 * {@link collectAnswers} (visibility/denylist/identity-exclusion via
 * {@link isCapturable}) but flipped to EMPTY instead of filled — in DOM
 * order. Shared by {@link collectQuestions} (produce the scan-time
 * correlation) and {@link locateQuestionField} (re-scan to fill), so the two
 * can never see a different candidate set.
 */
function emptyCandidateFields(doc: Document): HTMLElement[] {
  const out: HTMLElement[] = [];
  const fields = doc.querySelectorAll<HTMLElement>('input, textarea, select');

  for (const el of Array.from(fields)) {
    if (el instanceof HTMLInputElement) {
      if (!CAPTURABLE_INPUT_TYPES.has(el.type)) continue;
    } else if (!(el instanceof HTMLTextAreaElement) && !(el instanceof HTMLSelectElement)) {
      continue;
    }
    if (!isEmptyValue(el)) continue;
    if (!isCapturable(el)) continue;
    out.push(el);
  }

  return out;
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

/**
 * "Questions mode" for `answers.suggest`: scan `doc` for every EMPTY
 * candidate field (see {@link emptyCandidateFields}) and return its label
 * text plus the OCCURRENCE index among fields sharing that exact text — see
 * {@link ScannedQuestion}. Pure — no side effects.
 */
export function collectQuestions(doc: Document): ScannedQuestion[] {
  const out: ScannedQuestion[] = [];
  const counts = new Map<string, number>();

  for (const el of emptyCandidateFields(doc)) {
    const question = labelText(el).trim();
    if (!question) continue;
    const index = counts.get(question) ?? 0;
    counts.set(question, index + 1);
    out.push({ question, index });
  }

  return out;
}

/**
 * Re-scan `doc`'s CURRENT empty candidates (the identical predicate
 * {@link collectQuestions} used) and return the field at the `index`-th
 * occurrence of `question` — but ONLY when the CURRENT total number of
 * fields sharing that exact question text still equals `expectedCount` (the
 * count {@link collectQuestions} saw at scan time). Otherwise `null`.
 *
 * The count check matters beyond the obvious "field removed/filled" case: a
 * NEW same-labelled field inserted EARLIER in DOM order since the scan would
 * still leave occurrence `index` "in range" (there's still something at that
 * position), but it would now be a DIFFERENT field than the one scanned —
 * silently filling the wrong one. Requiring the total count to match closes
 * that gap: any insertion, removal, or relabeling changes the count and this
 * fails safe instead of ever returning a field the scan didn't see.
 *
 * Exported so `answer-fill.ts` (the classic-script injection entry) and its
 * tests can call it directly.
 */
export function locateQuestionField(
  doc: Document,
  question: string,
  index: number,
  expectedCount: number
): HTMLElement | null {
  const matches = emptyCandidateFields(doc).filter((el) => labelText(el).trim() === question);
  if (matches.length !== expectedCount) return null;
  return matches[index] ?? null;
}

/**
 * One scanned FILLED candidate field for "rewrite mode" (extension PR 11's
 * `answer.assist { mode: 'rewrite' }`) — the mirror of {@link ScannedQuestion}
 * (same occurrence-index correlation), plus the field's CURRENT text so the
 * popup can seed `existingAnswer` and offer "Restore original" without a
 * second scan. `answer` is page/user-derived and PII-adjacent (the user's
 * own past answer) — held only in memory for this popup session, never
 * written to `chrome.storage`.
 */
export interface FilledField {
  question: string;
  index: number;
  answer: string;
}

/**
 * Every FILLED, visible, labelled, non-ambiguous/non-identity candidate
 * field — TEXT INPUTS AND TEXTAREA ONLY (never `<select>`: a rewritten
 * free-text answer can't map onto a select's fixed options) — in DOM order.
 * Shares {@link isCapturable}'s visibility/denylist/identity gates with
 * {@link emptyCandidateFields}, flipped to FILLED. Shared by
 * {@link collectFilledFields} (produce the scan-time correlation) and
 * {@link locateFilledField} (re-scan to replace), so the two can never see a
 * different candidate set — same discipline
 * {@link collectQuestions}/{@link locateQuestionField} share.
 */
function filledCandidateFields(doc: Document): HTMLElement[] {
  const out: HTMLElement[] = [];
  const fields = doc.querySelectorAll<HTMLElement>('input, textarea');

  for (const el of Array.from(fields)) {
    if (el instanceof HTMLInputElement) {
      if (!CAPTURABLE_INPUT_TYPES.has(el.type)) continue;
    } else if (!(el instanceof HTMLTextAreaElement)) {
      continue;
    }
    if (isEmptyValue(el)) continue;
    if (!isCapturable(el)) continue;
    out.push(el);
  }

  return out;
}

/**
 * "Rewrite mode" scan: every FILLED candidate field (see
 * {@link filledCandidateFields}) with its current text and the OCCURRENCE
 * index among fields sharing that exact question text — see
 * {@link FilledField}. Pure — no side effects.
 */
export function collectFilledFields(doc: Document): FilledField[] {
  const out: FilledField[] = [];
  const counts = new Map<string, number>();

  for (const el of filledCandidateFields(doc)) {
    const question = labelText(el).trim();
    if (!question) continue;
    const answer =
      el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement ? el.value.trim() : '';
    if (!answer) continue;
    const index = counts.get(question) ?? 0;
    counts.set(question, index + 1);
    out.push({ question, index, answer });
  }

  return out;
}

/**
 * Re-scan `doc`'s CURRENT filled candidates (the identical predicate
 * {@link collectFilledFields} used) and return the field at the `index`-th
 * occurrence of `question` — but ONLY when the CURRENT total number of
 * fields sharing that exact question text still equals `expectedCount`.
 * Otherwise `null`. Mirrors {@link locateQuestionField}'s fail-safe
 * correlation exactly (same occurrence-index + expectedCount discipline),
 * over the FILLED candidate set instead of the empty one.
 *
 * Exported so `answer-fill.ts` (the classic-script injection entry) and its
 * tests can call it directly.
 */
export function locateFilledField(
  doc: Document,
  question: string,
  index: number,
  expectedCount: number
): HTMLElement | null {
  const matches = filledCandidateFields(doc).filter((el) => labelText(el).trim() === question);
  if (matches.length !== expectedCount) return null;
  return matches[index] ?? null;
}

/**
 * Passive "does this page have at least one fillable form field?" probe —
 * the union of {@link emptyCandidateFields} and {@link filledCandidateFields},
 * i.e. any labelled, visible, non-ambiguous/non-identity candidate field
 * regardless of its current EMPTY-vs-FILLED state. Reuses the SAME gates
 * "Save my answers"/"Suggest answers" already apply (no separate notion of
 * "fillable" is introduced) — never reads a field's VALUE beyond what those
 * collectors already need to classify it. Used to gate the popup's Form
 * group + Answer-tools disclosure: a plain job-listing page with no
 * application form has neither.
 */
export function hasFillableFields(doc: Document): boolean {
  return emptyCandidateFields(doc).length > 0 || filledCandidateFields(doc).length > 0;
}
