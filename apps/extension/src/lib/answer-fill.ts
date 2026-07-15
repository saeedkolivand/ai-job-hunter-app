/**
 * Pure "fill exactly one field" logic for `answers.suggest`'s per-row "Fill
 * this field" action.
 *
 * Locates the target field the SAME way the scan (`collectQuestions` in
 * `./answers-capture`) computed it — by (question label text, occurrence
 * index) — via the shared `locateQuestionField` re-scan, so a page mutation
 * between the scan and this click (a field filled/removed/relabeled) fails
 * safe: `{ filled: false, error }`, NEVER a fill onto a different field.
 *
 * Injected via `answer-fill.ts` (compiled to `answer-fill.js`, a classic
 * script — see `vite.config.ts`'s `injectedEntries` plugin), mirroring
 * `fill.ts`'s two-step register-then-invoke pattern: the answer text is the
 * user's own past application answer (PII-adjacent), so it is passed in
 * transiently via a second `executeScript({ func, args })` rather than baked
 * into the `files` injection — same discipline `fill.ts` uses for the
 * contact profile.
 *
 * Pure DOM — no extension APIs, no network — so it is unit-testable against a
 * jsdom document.
 */

import { locateFilledField, locateQuestionField } from './answers-capture';

/** Isolated-world global key `answer-fill.ts` exposes the filler under. MUST
 *  match the literal duplicated in `background.ts` (kept a plain literal
 *  there, not imported, so this module's runtime code never bundles into the
 *  background — same discipline as `autofill.ts`'s `AUTOFILL_GLOBAL`). */
export const ANSWER_FILL_GLOBAL = '__ajhRunAnswerFill';

/** Isolated-world global key `answer-replace.ts` exposes the replacer under
 *  — same discipline as {@link ANSWER_FILL_GLOBAL} (duplicated as a plain
 *  literal in `background.ts`, never imported there). */
export const ANSWER_REPLACE_GLOBAL = '__ajhRunAnswerReplace';

/** The fill outcome — see `answers.suggest`'s per-row Fill contract. */
export interface FillAnswerResult {
  filled: boolean;
  error?: string;
}

/** Fixed fail-safe result: the target field could not be re-located. Never a
 *  fallback to a different field. */
const NOT_FOUND: FillAnswerResult = {
  filled: false,
  error: 'Could not find this field — the page may have changed.',
};

/** Set a value the way a framework-controlled input notices (native setter +
 *  events) — mirrors `autofill.ts`'s private `setValue`. */
function setInputValue(el: HTMLInputElement | HTMLTextAreaElement, value: string): void {
  const proto = Object.getPrototypeOf(el) as object;
  const desc = Object.getOwnPropertyDescriptor(proto, 'value');
  if (desc?.set) desc.set.call(el, value);
  else el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
}

/** Select the `<select>` option whose VISIBLE text matches `value`
 *  (case-insensitive, trimmed) — a free-text answer can only fill a select
 *  when it names one of its options exactly; otherwise there is nothing safe
 *  to choose, so the caller must fail rather than guess. */
function setSelectValue(el: HTMLSelectElement, value: string): boolean {
  const target = value.trim().toLowerCase();
  const opt = Array.from(el.options).find((o) => o.text.trim().toLowerCase() === target);
  if (!opt) return false;
  el.value = opt.value;
  el.dispatchEvent(new Event('change', { bubbles: true }));
  return true;
}

/**
 * Locate the field at `(question, index)` among the CURRENT empty candidates
 * (via {@link locateQuestionField}) — refusing when the CURRENT count of
 * fields sharing `question` no longer equals `expectedCount` (the scan-time
 * count) — and fill it with `answer`. Fails safe — {@link NOT_FOUND} — on any
 * mutation since the scan (including a same-labelled field inserted
 * elsewhere on the page), or when the located field is a `<select>` whose
 * options don't include `answer`'s exact text.
 */
export function fillAnswerField(
  doc: Document,
  question: string,
  index: number,
  expectedCount: number,
  answer: string
): FillAnswerResult {
  const el = locateQuestionField(doc, question, index, expectedCount);
  if (!el) return NOT_FOUND;

  if (el instanceof HTMLSelectElement) {
    return setSelectValue(el, answer)
      ? { filled: true }
      : { filled: false, error: "Could not match this field's options." };
  }
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    setInputValue(el, answer);
    return { filled: true };
  }
  return NOT_FOUND;
}

/**
 * Locate the FILLED field at `(question, index)` among the CURRENT filled
 * candidates (via {@link locateFilledField}) — refusing when the CURRENT
 * count of fields sharing `question` no longer equals `expectedCount` (the
 * pick-time count) — and overwrite it with `text` (reusing
 * {@link setInputValue}'s native-setter + bubbling-events discipline).
 *
 * Backs BOTH extension PR 11 flows: Accept writes the rewritten draft,
 * Restore-original writes the SAME frozen text the field held at pick time —
 * the caller (background.ts) just passes a different `text`, there is no
 * separate "restore" code path. Text inputs/textarea ONLY (never `<select>`,
 * matching {@link locateFilledField}'s own scope) — fails safe
 * ({@link NOT_FOUND}) on any page mutation since the pick, exactly like
 * {@link fillAnswerField}. Never dispatches a submit.
 */
export function replaceFilledField(
  doc: Document,
  question: string,
  index: number,
  expectedCount: number,
  text: string
): FillAnswerResult {
  const el = locateFilledField(doc, question, index, expectedCount);
  if (!el) return NOT_FOUND;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    setInputValue(el, text);
    return { filled: true };
  }
  return NOT_FOUND;
}
