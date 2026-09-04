/**
 * ONE answer-tools state per (tab, origin) — the thing ADR-044 decision 1 puts
 * BEHIND both surfaces so the popup and the side panel are two views of it
 * rather than two features.
 *
 * ## Why it lives in `storage.session`, owned by the background
 *
 * The popup is torn down on blur, and blur is exactly what happens when the
 * user clicks into the page to paste an answer. The MV3 service worker is
 * evicted whenever idle. Neither is a place state can live. `storage.session`
 * is in-memory, survives the worker idling out, dies with the browser session
 * (so nothing the model wrote is ever persisted — ADR-033 is untouched), and
 * emits `storage.onChanged` to EVERY extension context, which is what makes a
 * stream started in the popup already be on screen in the panel.
 *
 * ## Why the key is the tab id and the origin rides inside the record
 *
 * `origin` is captured from the gesture-granted tab AT GESTURE TIME (the tab's
 * url is readable then because `activeTab` was just granted), never through a
 * `tabs`-permission lookup — `tabs` is on `manifest.test.ts`'s denylist and
 * stays there. Keying by tab id alone is what lets the rows SURVIVE a
 * cross-origin navigation, which decision 3 requires: the rows stay on screen,
 * {@link AnswerState.pageChanged} flips, and every control that would read or
 * write the page is replaced by one line asking for the toolbar click. The
 * record is still the state OF that (tab, origin) pair — `origin` is what says
 * which pair, and a mismatch is what invalidates the write controls.
 *
 * Everything in this module above the storage helpers is pure, so the row
 * model can be tested without a browser.
 */

import { type Browser, browser } from '@wxt-dev/browser';

import type { FilledField, ScannedQuestion } from './answers-capture';

/** `storage.session` key prefix — one entry per tab. */
const STATE_KEY_PREFIX = 'answerState:';

/** The `storage.session` key holding `tabId`'s answer state. */
export function answerStateKey(tabId: number): string {
  return `${STATE_KEY_PREFIX}${tabId}`;
}

/**
 * Which candidate set a row's field came from, and therefore which fail-safe
 * re-locate an Accept must use: `empty` rows go through
 * `locateQuestionField`/`answer-fill.js`, `filled` rows through
 * `locateFilledField`/`answer-replace.js`. The two sets have SEPARATE
 * occurrence-index namespaces (see `answers-capture.ts`), so a row that lost
 * track of which one it belongs to could correlate to a different field
 * entirely — hence this is part of the reference, not a rendering detail.
 */
export type AnswerFieldKind = 'empty' | 'filled';

/** The scan-time correlation for the one field a row acts on. */
export interface AnswerFieldRef {
  kind: AnswerFieldKind;
  /** Occurrence index among fields sharing this exact question text. */
  index: number;
  /** How many fields shared that text AT SCAN TIME — the fail-safe count. */
  count: number;
  /** The field's own `maxlength`, when it declares one. */
  maxChars?: number;
  /**
   * What we believe the field CURRENTLY holds: the scanned text for a filled
   * field, `''` for an empty one, and whatever a successful Accept/Restore
   * last wrote. `replaceFilledField` refuses when the field's real text no
   * longer matches, so this is what keeps an Accept from clobbering a manual
   * edit made since the scan.
   */
  currentText: string;
  /**
   * The field's text AT SCAN TIME, frozen. `currentText` moves with every
   * accepted write; this never does, so "Restore original" always has the
   * user's own words to put back even after several accepts. Empty for an
   * empty field, which is exactly what restoring one should write.
   */
  originalText: string;
}

/**
 * A row's headline state. `saved-available` is an EMPTY field for which a past
 * application already has an answer — the one status that is about data the
 * extension holds rather than about the page.
 */
export type AnswerRowStatus = 'empty' | 'saved-available' | 'filled' | 'drafted';

/**
 * The wire's own grounding flags, copied verbatim off
 * `ExtensionAnswerAssistResult.sourced` so the "grounded on" line can only
 * ever claim what the desktop actually reported. The résumé is NOT a flag
 * there — draft mode always grounds on it — so the line adds it itself and a
 * REWRITE (where every flag is false and no résumé is read) gets no line at
 * all rather than a false one.
 */
export interface AnswerSourced {
  /** The opt-in web-search notes were fetched and non-empty. */
  web: boolean;
  /** A cached company brief from the matched Application was used. */
  brief: boolean;
  /** This question routed the salary-shaped path. */
  salary: boolean;
}

/** One session-only version of a row's answer (ADR-044 decision 5). */
export interface AnswerVersion {
  /** `v1`, `v2`, … — the label the version tabs render. */
  label: string;
  text: string;
  /**
   * How this version was produced. `draft` is a fresh grounded draft
   * (Regenerate); `rewrite` is a reshape of the PREVIOUS version through the
   * wire's rewrite mode, which never sees the résumé or the posting. The UI
   * says which is which instead of presenting them as one dial.
   */
  kind: 'draft' | 'rewrite';
  /** The `sourced` flags the desktop reported, for the "grounded on" line.
   *  Only a `draft` version has them — a rewrite is grounded on nothing. */
  sourced?: AnswerSourced;
}

/** One application question, with everything the composer needs. */
export interface AnswerRow {
  /** Stable across rescans so a drafted row keeps its versions. */
  id: string;
  question: string;
  /** `null` for a free-text row: a question the scan missed, or one pulled
   *  from a context-menu selection. Such a row can be drafted and copied but
   *  NEVER accepted — there is no field on the page to write it into. */
  field: AnswerFieldRef | null;
  status: AnswerRowStatus;
  versions: AnswerVersion[];
  /** Index into {@link versions}, or `-1` for the page's own text. */
  selected: number;
  /** An answer the user gave to this same question on a PAST application,
   *  offered as-is (today's behaviour, ADR-044 decision 4). Present only on a
   *  `saved-available` row. */
  savedAnswer?: string;
  /** Where {@link savedAnswer} came from, shown beside it so a reused answer
   *  is never mistaken for one written for THIS application. */
  savedSource?: string;
  /** The last refusal/failure for this row, rendered verbatim. */
  error?: string;
  /**
   * A neutral (non-error) note about the row's last action — e.g. a rewrite
   * that came back unchanged. Rendered with the same muted styling as the
   * "page changed" line, never the error styling: nothing failed, so nothing
   * should look like it did. Cleared wherever {@link error} is cleared or set.
   */
  notice?: string;
}

/** The one in-flight (or last-finished) stream, tagged with its row. */
export interface AnswerStream {
  rowId: string;
  text: string;
  done: boolean;
  interrupted: boolean;
  /** `rewrite` streams reshape; `draft` streams are grounded. Kept here so a
   *  view that attaches mid-stream can already say which it is watching. */
  kind: 'draft' | 'rewrite';
}

/** The whole per-(tab, origin) state both surfaces render. */
export interface AnswerState {
  tabId: number;
  /** Captured from the gesture-granted tab at gesture time. Never a lookup. */
  origin: string;
  scannedAt: number;
  rows: AnswerRow[];
  stream: AnswerStream | null;
  /**
   * The tab navigated since the scan, so the `activeTab` grant may be gone and
   * the scanned rows may no longer correspond to anything on screen. Set
   * conservatively: without the `tabs` permission the background cannot read
   * the new url, so ANY navigation in the tab flips this, including a
   * same-origin one that would in fact have kept the grant. Under-claiming is
   * the safe direction — the cost is one extra toolbar click, and the
   * alternative is offering a write control that silently does nothing.
   */
  pageChanged: boolean;
}

// ── pure row model ────────────────────────────────────────────────────────────

/** Row id for a scanned field. Stable across rescans of the same page. */
function fieldRowId(kind: AnswerFieldKind, question: string, index: number): string {
  return `${kind}:${index}:${question}`;
}

/** Row id for a free-text row — content-keyed, so adding the same question
 *  twice (e.g. two context-menu clicks on the same selection) reuses the row
 *  rather than stacking duplicates. */
export function freeRowId(question: string): string {
  return `${FREE_ROW_PREFIX}${question.trim()}`;
}

/** Id prefix that marks a row as user-typed rather than scanned. */
const FREE_ROW_PREFIX = 'free:';

/** Occurrences of each question text within one candidate set. */
function countsByQuestion(entries: { question: string }[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const e of entries) counts.set(e.question, (counts.get(e.question) ?? 0) + 1);
  return counts;
}

/** The raw scan `capture-rows.js` hands back. */
export interface AnswerScan {
  questions: ScannedQuestion[];
  filled: FilledField[];
}

/** The headline status for a freshly-scanned row. */
function rowStatus(
  kind: AnswerFieldKind,
  hasVersions: boolean,
  hasSaved: boolean
): AnswerRowStatus {
  if (hasVersions) return 'drafted';
  if (kind === 'filled') return 'filled';
  return hasSaved ? 'saved-available' : 'empty';
}

/**
 * Rebuild the row list from a fresh scan, carrying over the versions of any
 * row the scan still finds.
 *
 * A rescan is a mid-form event (a multi-step form advanced, a field got
 * filled) — dropping a drafted answer because its field flipped from empty to
 * filled would throw away exactly the work the user asked for. So versions,
 * selection and error are preserved BY ROW ID, and only the field reference
 * and status are re-derived. A row whose field disappeared entirely survives
 * as a FREE-TEXT row when it holds versions (its drafts are still worth
 * copying) and is dropped when it holds none.
 *
 * `savedFor` maps a question text to the answer a PAST application already
 * has for it — the `saved-available` status. An empty map is fine; the status
 * simply stays `empty`, which is what a desktop that refused the lookup (or
 * one that is not paired) leaves behind.
 *
 * Pure — no DOM, no storage.
 */
export function buildRows(
  scan: AnswerScan,
  savedFor: ReadonlyMap<string, { answer: string; source?: string }>,
  previous: readonly AnswerRow[] = []
): AnswerRow[] {
  const carried = new Map(previous.map((r) => [r.id, r]));
  const emptyCounts = countsByQuestion(scan.questions);
  const filledCounts = countsByQuestion(scan.filled);
  const rows: AnswerRow[] = [];
  const seen = new Set<string>();

  const push = (
    kind: AnswerFieldKind,
    question: string,
    index: number,
    count: number,
    maxChars: number | undefined,
    currentText: string
  ): void => {
    const id = fieldRowId(kind, question, index);
    if (seen.has(id)) return;
    seen.add(id);
    const prior = carried.get(id);
    const field: AnswerFieldRef = { kind, index, count, currentText, originalText: currentText };
    if (maxChars !== undefined) field.maxChars = maxChars;
    const versions = prior?.versions ?? [];
    const saved = savedFor.get(question);
    rows.push({
      id,
      question,
      field,
      status: rowStatus(kind, versions.length > 0, saved !== undefined),
      versions,
      selected: prior?.selected ?? -1,
      ...(saved === undefined
        ? {}
        : { savedAnswer: saved.answer, ...(saved.source ? { savedSource: saved.source } : {}) }),
      ...(prior?.error === undefined ? {} : { error: prior.error }),
      ...(prior?.notice === undefined ? {} : { notice: prior.notice }),
    });
  };

  for (const q of scan.questions) {
    push('empty', q.question, q.index, emptyCounts.get(q.question) ?? 1, q.maxChars, '');
  }
  for (const f of scan.filled) {
    push('filled', f.question, f.index, filledCounts.get(f.question) ?? 1, f.maxChars, f.answer);
  }

  // Rows the scan no longer sees. Two survive, as free text: one carrying
  // drafted work (throwing away an answer because its field moved is the one
  // thing a rescan must never do), and one the user TYPED themselves — a
  // question they added deliberately does not stop being their question just
  // because the page never had a field for it. Everything else is a scanned
  // row that is simply gone, and keeping it would be inventing a question.
  for (const prior of previous) {
    if (seen.has(prior.id)) continue;
    if (prior.versions.length === 0 && !prior.id.startsWith(FREE_ROW_PREFIX)) continue;
    rows.push({ ...prior, field: null, status: prior.versions.length > 0 ? 'drafted' : 'empty' });
  }

  return rows;
}

/**
 * Add (or reuse) a free-text row for a question the scan missed — the manual
 * entry and the context-menu selection both land here. Returns an equivalent
 * array when the row already exists, so a repeated gesture is idempotent.
 *
 * Pure — returns a new array, never mutates.
 */
export function addFreeRow(rows: readonly AnswerRow[], question: string): AnswerRow[] {
  const trimmed = question.trim();
  if (!trimmed) return [...rows];
  const id = freeRowId(trimmed);
  if (rows.some((r) => r.id === id)) return [...rows];
  return [
    { id, question: trimmed, field: null, status: 'empty', versions: [], selected: -1 },
    ...rows,
  ];
}

/** Append a finished version to a row and select it. Pure. */
export function appendVersion(
  rows: readonly AnswerRow[],
  rowId: string,
  text: string,
  kind: 'draft' | 'rewrite',
  sourced?: AnswerSourced
): AnswerRow[] {
  return rows.map((row) => {
    if (row.id !== rowId) return row;
    const version: AnswerVersion = {
      label: `v${row.versions.length + 1}`,
      text,
      kind,
      ...(sourced === undefined ? {} : { sourced }),
    };
    const versions = [...row.versions, version];
    const next: AnswerRow = { ...row, versions, selected: versions.length - 1, status: 'drafted' };
    delete next.error;
    delete next.notice;
    return next;
  });
}

/**
 * Normalise for the unchanged-rewrite comparison: collapse every whitespace
 * run to a single space, then drop ONLY trailing punctuation (`\p{P}` —
 * commas, periods, quotes — Unicode category `Po`/`Ps`/`Pe`/…, which does NOT
 * include a math/currency symbol like `+`/`$`). Deliberately NOT `\p{S}`: the
 * desktop's twin helper (`apps/desktop/src/renderer/lib/generate/rewrite.ts`)
 * strips `\p{P}\p{S}` together, which misclassifies a genuinely different
 * result ("20" vs "20+", meaning "at least 20") as unchanged. Also
 * deliberately NOT case-folding — "make this all caps" is a real rewrite
 * whose result must still count as changed.
 */
export function normalizeAnswerText(text: string): string {
  return text
    .replace(/\s+/gu, ' ')
    .trim()
    .replace(/\p{P}+$/u, '')
    .trim();
}

/**
 * True when a chip-rewrite came back the same text it started from (ignoring
 * whitespace and trailing punctuation) — a no-op the desktop's stream reports
 * as a success. An empty `previous` is never "unchanged": there is nothing to
 * compare against.
 */
export function isUnchangedRewrite(previous: string, next: string): boolean {
  const before = normalizeAnswerText(previous);
  return before.length > 0 && before === normalizeAnswerText(next);
}

/** The text a row currently shows: the selected version, else the page text. */
export function selectedText(row: AnswerRow): string {
  const version = row.versions[row.selected];
  return version ? version.text : (row.field?.currentText ?? '');
}

/** The version a rewrite chip reshapes — always the LATEST one, never the
 *  selected one: restoring v1 to read it and then pressing a chip should not
 *  silently throw v2 away (Restore is how you go back, a chip is how you go
 *  forward). Falls back to the page's own text for a not-yet-drafted row. */
export function rewriteBaseText(row: AnswerRow): string {
  const last = row.versions[row.versions.length - 1];
  return last ? last.text : (row.field?.currentText ?? '');
}

/** Whether a row may be accepted into the page: it must HAVE a field, the page
 *  must not have changed under it, and there must be text to write. A drafted
 *  answer with nowhere to go is Copy-only — the honest state decision 4 asks
 *  for. */
export function canAccept(row: AnswerRow, pageChanged: boolean): boolean {
  return row.field !== null && !pageChanged && selectedText(row).trim().length > 0;
}

/** Live "n / limit" counter text, or `null` when the field declares no limit.
 *  Counts the text on screen, never what the model claims it produced. */
export function counterText(row: AnswerRow, text: string): string | null {
  const limit = row.field?.maxChars;
  if (limit === undefined) return null;
  return `${text.length} / ${limit} characters`;
}

/** Whether `text` overruns the row's own limit — what gates the fit-the-limit
 *  chip and the over-limit warning. `false` when there is no limit. */
export function isOverLimit(row: AnswerRow, text: string): boolean {
  const limit = row.field?.maxChars;
  return limit !== undefined && text.length > limit;
}

// ── storage.session accessors (the only impure part) ─────────────────────────

/**
 * `storage.session` as a plain area handle. Firefox exposes it too, but it is
 * absent on older engines and (on Chrome) in contexts without the session
 * area, so every accessor degrades to "no state" rather than throwing — a
 * missing session area must cost the panel its memory, never its render.
 */
function sessionArea(): Browser.storage.StorageArea | null {
  const area = (browser.storage as { session?: Browser.storage.StorageArea }).session;
  return area ?? null;
}

/** Read the answer state for `tabId`, or `null` when there is none. */
export async function readAnswerState(tabId: number): Promise<AnswerState | null> {
  const area = sessionArea();
  if (!area) return null;
  const key = answerStateKey(tabId);
  try {
    const stored = await area.get(key);
    const value = stored[key];
    return isAnswerState(value) ? value : null;
  } catch {
    return null;
  }
}

/** Write the answer state for its own tab. Every view sees the change. */
export async function writeAnswerState(state: AnswerState): Promise<void> {
  const area = sessionArea();
  if (!area) return;
  try {
    await area.set({ [answerStateKey(state.tabId)]: state });
  } catch {
    // Session storage is best-effort: an over-quota or missing area must not
    // fail the user's click, it just costs the reattach.
  }
}

/** Drop a tab's state (the tab closed). */
export async function clearAnswerState(tabId: number): Promise<void> {
  const area = sessionArea();
  if (!area) return;
  try {
    await area.remove(answerStateKey(tabId));
  } catch {
    // Same best-effort rationale as `writeAnswerState`.
  }
}

/**
 * Read-modify-write one tab's state under the background's own single-threaded
 * event loop. Returns the written state, or `null` when `mutate` declined (it
 * returned `null`) or there was nothing to mutate.
 */
export async function updateAnswerState(
  tabId: number,
  mutate: (state: AnswerState) => AnswerState | null
): Promise<AnswerState | null> {
  const current = await readAnswerState(tabId);
  if (!current) return null;
  const next = mutate(current);
  if (!next) return null;
  await writeAnswerState(next);
  return next;
}

/**
 * Minimal shape guard for a value read back out of `storage.session`. Not
 * defence against an attacker (nothing else can write this area) — defence
 * against a stale record written by a PREVIOUS extension version whose shape
 * has since changed, which would otherwise crash the renderer on upgrade.
 */
function isAnswerState(v: unknown): v is AnswerState {
  if (typeof v !== 'object' || v === null) return false;
  const s = v as Record<string, unknown>;
  return (
    typeof s.tabId === 'number' &&
    typeof s.origin === 'string' &&
    typeof s.pageChanged === 'boolean' &&
    Array.isArray(s.rows)
  );
}

/**
 * Subscribe a VIEW to one tab's state. Calls `onState` immediately with the
 * current value, then on every `storage.session` change to that tab's key.
 * Returns an unsubscribe function.
 *
 * This is the whole of ADR-044 decision 1's "two views, one state": both
 * surfaces call this, so a stream started in either is rendered by both and
 * closing the popup loses nothing.
 */
export function subscribeAnswerState(
  tabId: number,
  onState: (state: AnswerState | null) => void
): () => void {
  const key = answerStateKey(tabId);
  const listener = (
    changes: Record<string, Browser.storage.StorageChange>,
    areaName: string
  ): void => {
    if (areaName !== 'session' || !(key in changes)) return;
    const value = changes[key]?.newValue;
    onState(isAnswerState(value) ? value : null);
  };

  browser.storage.onChanged.addListener(listener);
  void readAnswerState(tabId).then(onState);
  return () => browser.storage.onChanged.removeListener(listener);
}
