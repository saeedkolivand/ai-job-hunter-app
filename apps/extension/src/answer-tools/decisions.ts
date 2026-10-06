/**
 * Pure copy + decisions for the Answer-tools section: what each row says and
 * which chips it offers. No DOM, no I/O — everything here is unit-testable on
 * its own and re-exported unchanged from `answer-tools.ts`.
 */

// The dedicated `extension-protocol` entrypoint, not the `@ajh/shared` barrel:
// this is a RUNTIME import, and the barrel drags zod and the whole IPC surface
// into an MV3 bundle that must stay reviewable and small (measured: 98.8 kB of
// chunk vs 4.2 kB). Same reason `lib/bridge/` imports from there. TYPE-only
// imports may keep using the barrel — they are erased at build.
import {
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_NO_PROVIDER_MESSAGE,
  type ExtensionRewritePreset,
} from '@ajh/shared/extension-protocol';

import { type AnswerRow, type AnswerState, canAccept, isOverLimit } from '../lib/answer-state';

/**
 * The two shared refusal sentinels, as a set. The desktop's wire-error
 * discipline is fixed sentinel TEXT rather than a machine-readable code (the
 * sentinel IS the code — `docs/knowledge/extension-domain.md`), and these are
 * the constants the desktop declares beside the handler, so matching them is
 * matching the source of truth rather than a copied string.
 */
const REFUSAL_SENTINELS: ReadonlySet<string> = new Set([
  EXTENSION_AI_ASSIST_OFF_MESSAGE,
  EXTENSION_NO_PROVIDER_MESSAGE,
]);

/**
 * What a gated-off row should say, or `null` when this error is not one of the
 * two sentinels and must therefore be rendered verbatim like every other
 * opaque wire error.
 *
 * The sentinel text already names the setting to turn on, so this adds only
 * the part the desktop cannot know: which of THIS row's controls keep working
 * while drafting is off. Saying "AI is off" and leaving the row looking dead
 * is the failure mode being avoided.
 */
export function gatedOffNotice(error: string | undefined): string | null {
  if (error === undefined || !REFUSAL_SENTINELS.has(error)) return null;
  return `${error} Saved answers, the version history and the character counter keep working while drafting is off.`;
}

/** The badge text for a row's status. */
export function statusBadge(row: AnswerRow): string {
  switch (row.status) {
    case 'saved-available':
      return 'Saved answer';
    case 'filled':
      return 'Filled';
    case 'drafted':
      return `${row.versions[row.versions.length - 1]?.label ?? 'v1'} ready`;
    default:
      return row.field === null ? 'Not on page' : 'Empty';
  }
}

/**
 * The Accept sentence, naming the EXACT question it overwrites (ADR-044
 * decision 4's honesty requirement) — `null` when there is no Accept to
 * explain, so the sentence can never appear beside a disabled or absent
 * button and promise something that will not happen.
 */
export function acceptSentence(row: AnswerRow, pageChanged: boolean): string | null {
  if (!canAccept(row, pageChanged)) return null;
  return `Accept replaces the answer in “${row.question}” on the page. Nothing else is touched.`;
}

/**
 * The "grounded on" line for the version on screen, built from the wire's own
 * `sourced` flags plus the résumé (which draft mode always uses and therefore
 * never reports as a flag). `null` for a REWRITE and for the page's own text:
 * a rewrite is a pure text transform that never sees the résumé or the
 * posting, so claiming grounding for it would be the exact overstatement
 * decision 5 exists to stop.
 */
export function groundedOnLine(row: AnswerRow): string | null {
  const version = row.versions[row.selected];
  if (!version || version.kind !== 'draft') return null;
  const parts = ['your résumé'];
  if (version.sourced?.brief) parts.push('this posting');
  if (version.sourced?.salary) parts.push('the saved salary range');
  if (version.sourced?.web) parts.push('web search');
  return `Grounded on: ${parts.join(' · ')}`;
}

/**
 * The one honest line about what the chips do, versus Regenerate — a
 * function of the ROW rather than a flat constant, because the "and this
 * posting" clause is only true when the row's own selected draft actually
 * used one ({@link groundedOnLine}'s same `sourced.brief` signal). A row
 * drafted before a job was matched must not claim grounding Regenerate would
 * not actually have.
 */
export function iterationHint(row: AnswerRow): string {
  const version = row.versions[row.selected];
  const groundedOnPosting = version?.kind === 'draft' && version.sourced?.brief === true;
  return groundedOnPosting
    ? 'Chips reshape this text. Regenerate rethinks it from your résumé and this posting.'
    : 'Chips reshape this text. Regenerate rethinks it from your résumé.';
}

/** The line that replaces every write control after a navigation. */
export const PAGE_CHANGED_LINE =
  'This page changed. Click the toolbar icon to re-grant access and scan it — your drafts below are kept.';

/** Header summary: how many questions, and how many still need an answer. */
export function summaryLine(state: AnswerState | null): string {
  if (!state || state.rows.length === 0) return 'Nothing scanned yet';
  const total = state.rows.length;
  const answered = state.rows.filter((r) => r.status === 'filled' || r.status === 'drafted').length;
  const noun = total === 1 ? 'question' : 'questions';
  return `${total} ${noun} · ${total - answered} to go`;
}

/** One rewrite chip: a label plus what it sends over the existing wire verb. */
export interface RewriteChip {
  label: string;
  preset?: ExtensionRewritePreset;
  instruction?: string;
}

/**
 * TONE chips. Every one is a REWRITE of the latest version through the wire's
 * existing rewrite mode — no protocol change (decision 5). Two of them map to
 * a server-side preset; the rest carry a free instruction, because there is no
 * preset for "warmer". The server-side resolver COMBINES a preset with typed
 * free text (or combines the chip instruction with typed text client-side for
 * instruction chips — `renderChipRow`), so neither side of the pair discards
 * the other (issue 1231).
 *
 * The leading "As is" is deliberate and does nothing: without an explicit
 * neutral the chip row reads as a required choice, and a user who likes the
 * tone has to guess that not pressing anything is allowed.
 */
export const TONE_CHIPS: readonly RewriteChip[] = [
  { label: 'As is' },
  {
    label: 'Warmer',
    instruction: 'Make this warmer and more personal, keeping every concrete fact.',
  },
  {
    label: 'Formal',
    instruction: 'Make this more formal and professional, keeping every concrete fact.',
  },
  {
    label: 'Simpler',
    instruction: 'Make this plainer and easier to read, keeping every concrete fact.',
  },
  { label: 'More impact', preset: 'impact' },
  { label: 'Fix grammar', preset: 'grammar' },
];

/** LENGTH chips. Same rewrite path as {@link TONE_CHIPS}, same explicit
 *  "As is". The fit-the-limit chip is added separately because it only exists
 *  when there IS a limit and the text is over it. */
export const LENGTH_CHIPS: readonly RewriteChip[] = [
  { label: 'As is' },
  { label: 'Shorter', preset: 'shorten' },
  { label: 'Longer', preset: 'expand' },
];

/**
 * The fit-the-limit chip for a row that is over its field's own `maxlength`,
 * or `null` when there is no limit or the text already fits. It carries the
 * MEASURED overshoot rather than asking the model to count: the count is
 * taken here, from the text on screen.
 */
export function fitLimitChip(row: AnswerRow, text: string): RewriteChip | null {
  const limit = row.field?.maxChars;
  if (limit === undefined || !isOverLimit(row, text)) return null;
  return {
    label: `Fit ${limit}`,
    instruction: `This is ${text.length} characters; the limit is ${limit}. Cut at least ${text.length - limit} characters, keeping every concrete fact.`,
  };
}
