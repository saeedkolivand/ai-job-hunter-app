/**
 * Pure helpers for the inline rewrite popover (F4). Three concerns, all free of
 * IPC, React and the preferences store so the popover can import them directly
 * (not through the `@/lib/generate` barrel) and a component test that stubs the
 * barrel's IPC-backed exports still exercises the real logic:
 *
 *  1. {@link isUnchangedRewrite} — a rewrite that comes back identical to the
 *     selection is a no-op, not a success. Measured 2026-09-03: the "shorten"
 *     preset returned the selection verbatim 3/3 on a 130-char sentence and the
 *     UI offered Accept as if something had happened.
 *  2. {@link parseRewriteLimit} / {@link measureRewriteLength} — a numeric
 *     length limit in the instruction is verified by CODE, never trusted from
 *     the model. Measured on a 677-char span: "under 200 characters" landed at
 *     199-211 (crossing the limit about half the time) and "at most 40 words"
 *     at 41-46 (0 of 3 inside). Same rule as the backend's exactly-one-re-ask
 *     (`pipeline/mod.rs`).
 *  3. {@link deriveRewriteLocale} — the language of the SPAN, not of the
 *     document. Measured: a Dutch span inside a document whose
 *     `meta.targetLanguage` is `en` came back in English in 12 of 18 runs.
 */

import { detectLanguage } from '@ajh/shared/language-detection';

// ─── Unchanged-result predicate ───────────────────────────────────────────────

/**
 * Normalise for the unchanged comparison: collapse every whitespace run to a
 * single space, then drop trailing punctuation/symbols. Deliberately NOT
 * case-folding — "make this all caps" is a real instruction whose result must
 * still count as changed.
 */
export function normalizeRewriteText(text: string): string {
  return text
    .replace(/\s+/gu, ' ')
    .trim()
    .replace(/[\p{P}\p{S}]+$/u, '')
    .trim();
}

/**
 * True when the model handed back the selection again (ignoring whitespace and
 * trailing punctuation). An empty selection is never "unchanged" — there is
 * nothing to compare against.
 */
export function isUnchangedRewrite(selection: string, result: string): boolean {
  const before = normalizeRewriteText(selection);
  return before.length > 0 && before === normalizeRewriteText(result);
}

// ─── Numeric length limit ─────────────────────────────────────────────────────

/** The unit a parsed limit counts in. */
export type RewriteLimitUnit = 'chars' | 'words';

export interface RewriteLimit {
  unit: RewriteLimitUnit;
  /** The maximum, inclusive: a result of exactly `max` units is inside it. */
  max: number;
}

/**
 * `<number> <unit>` anywhere in the instruction. English + German unit words,
 * because the preset instructions and the whole UI ship in both locales.
 */
const LIMIT_PATTERN = /(\d{1,6})\s*(characters?|chars?|zeichen|words?|wörter|worte|wort)/giu;

/**
 * Phrases that make the number a FLOOR rather than a ceiling. Matched against
 * the text immediately before the number, so "at least 200 characters" yields
 * no limit instead of a limit the rewrite is supposed to exceed.
 */
const MINIMUM_PREFIX =
  /(at least|no fewer than|not fewer than|minimum(?: of)?|min\.?|more than|over|longer than|mindestens|wenigstens|mehr als)\s*$/iu;

/** How much text before a number is inspected for a {@link MINIMUM_PREFIX}. */
const PREFIX_WINDOW = 24;

/**
 * Parse the binding numeric length limit out of a rewrite instruction, or
 * `null` when it carries none (every preset except a free-text one: "Cut this
 * to about two thirds of its length" is a proportion, not a number).
 *
 * When several numbers appear, CHARACTERS win over words (an exact character
 * count is what the popover can verify against the field the user is looking
 * at), and within one unit the SMALLEST number wins — "under 200 characters,
 * ideally around 150" is a 150-character limit, the one that satisfies both.
 */
export function parseRewriteLimit(instruction: string): RewriteLimit | null {
  let chars: number | null = null;
  let words: number | null = null;

  for (const match of instruction.matchAll(LIMIT_PATTERN)) {
    const value = Number(match[1]);
    if (!Number.isFinite(value) || value <= 0) continue;
    const at = match.index ?? 0;
    if (MINIMUM_PREFIX.test(instruction.slice(Math.max(0, at - PREFIX_WINDOW), at))) continue;
    // Every word-unit spelling above starts with "w" (words / wörter / worte /
    // wort); every character-unit one does not (characters / chars / zeichen).
    const unit: RewriteLimitUnit = /^w/iu.test(match[2] ?? '') ? 'words' : 'chars';
    if (unit === 'chars') chars = chars === null ? value : Math.min(chars, value);
    else words = words === null ? value : Math.min(words, value);
  }

  if (chars !== null) return { unit: 'chars', max: chars };
  if (words !== null) return { unit: 'words', max: words };
  return null;
}

/** Count `text` in `unit` — the same count the over-limit line shows the user. */
export function measureRewriteLength(text: string, unit: RewriteLimitUnit): number {
  const trimmed = text.trim();
  if (unit === 'chars') return trimmed.length;
  return trimmed ? trimmed.split(/\s+/u).length : 0;
}

/** True when `text` is longer than `limit` allows. */
export function exceedsRewriteLimit(text: string, limit: RewriteLimit): boolean {
  return measureRewriteLength(text, limit.unit) > limit.max;
}

// ─── Span language ────────────────────────────────────────────────────────────

/**
 * The language the rewrite must come back in: the SELECTION's own detected
 * language, falling back to `fallback` (the document's `meta.targetLanguage`)
 * only when detection is unsure. `detectLanguage` returns `'unknown'` for a
 * span under 20 characters or an undetermined script, which is exactly the
 * "unsure" case — a short span is best served by the document's language.
 */
export function deriveRewriteLocale(selection: string, fallback = 'en'): string {
  const detected = detectLanguage(selection);
  return detected === 'unknown' ? fallback || 'en' : detected;
}

/**
 * The ONE re-ask instruction for a result that broke a parsed numeric limit:
 * the original instruction plus the MEASURED overshoot, so the model is told
 * the number it missed instead of being asked to try harder. English on
 * purpose — it is appended to the English rewrite system prompt
 * (`packages/prompts/src/generate/rewrite/rewrite.ts`), which is what the model
 * reads; the span's own language is pinned separately by rule 7.
 *
 * Exactly one re-ask, never a loop: the same rule the backend pipeline applies
 * to its own verified stages. If the second attempt is still over, the popover
 * shows the count next to Accept and the user decides.
 */
export function buildOvershootInstruction(
  instruction: string,
  limit: RewriteLimit,
  actual: number
): string {
  const unitWord = limit.unit === 'chars' ? 'characters' : 'words';
  const cut = Math.max(1, actual - limit.max);
  return `${instruction}\n\nYour previous attempt was too long: this is ${actual} ${unitWord}; the limit is ${limit.max} ${unitWord}; cut at least ${cut} ${unitWord}, keep every fact.`;
}
