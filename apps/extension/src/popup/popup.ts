/**
 * Popup controller (plain TS — deliberately NOT the app's React stack).
 *
 * It is a thin view over the background worker: it sends typed
 * {@link PopupRequest}s, renders the {@link ConnectionStatus} the background
 * returns/pushes, and never talks to the desktop bridge directly. Store
 * reviewers test WITHOUT the desktop app, so every state must render an
 * explanation, never an error.
 */

import { browser } from '@wxt-dev/browser';

import type { ExtensionAnswerSuggestion, ExtensionRewritePreset } from '@ajh/shared';

import type { FilledField, ScannedQuestion } from '../lib/answers-capture';
import type { ConnectionStatus, PopupRequest, PopupResponse } from '../lib/messages';
import { getAnswerToolsExpanded, looksLikeToken, setAnswerToolsExpanded } from '../lib/storage';

import './popup.css';

// ── pure view-decision helpers (exported for unit tests) ─────────────────────

/**
 * Given a `getStatus` response, return the {@link ConnectionStatus} to render,
 * or `null` if the response signals the background is unreachable (use offline
 * fallback in that case).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveStatusResponse(
  res: PopupResponse,
  lastKnownHasToken: boolean
): ConnectionStatus {
  if (res.ok && res.kind === 'status') return res.status;
  // `!ok` or unexpected kind → offline fallback preserving last-known token.
  return { phase: 'app_not_running', port: null, hasToken: lastKnownHasToken };
}

/** Where an imported job lands in the desktop app — shown on success so the
 *  user knows where to look (the extension can't focus the native window). */
const IMPORT_LANDING_HINT = 'Open AI Job Hunter → Applications to view it.';

/** Shown when the job was saved but the description couldn't be read. */
const IMPORT_PARTIAL_HINT = 'Open AI Job Hunter → Applications to paste it.';

/** Percent-fit suffix appended to the import success/status-unchanged lines
 *  when the desktop populated `matchScore` (a best-effort keyword-only score,
 *  omitted on failure — see `ExtensionImportResult`'s doc) — mirrors the
 *  "Check fit" card's percent treatment (`resolveMatchLiveResponse`) without
 *  the résumé name the import reply doesn't carry. Absent field → empty
 *  string, so the message is byte-identical to before this field existed. */
function matchScoreSuffix(matchScore: number | undefined): string {
  return typeof matchScore === 'number' ? ` — ${Math.round(matchScore)}% fit.` : '';
}

/**
 * Given an `import` response, return the message text and tone to display. On
 * success it names the imported job (when the desktop parsed a title) and points
 * the user at where it landed, instead of a bare “Imported”.
 *
 * `requestedApplied` is the "I already applied" checkbox state sent with the
 * request. The desktop dedup-merges by URL and only ever advances a matched
 * Application's status OUT of `saved` — it never demotes an existing
 * applied-or-further row. So when the checkbox was NOT ticked and the matched
 * row's status is already past `saved`, a bare "Imported" success would read
 * like the status had changed when only the status was left untouched (the
 * desktop meta merge still refreshes title/company/description/answers) —
 * surface that explicitly instead.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveImportResponse(
  res: PopupResponse,
  requestedApplied: boolean
): { text: string; tone: 'ok' | 'err' } {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'import') return { text: 'Unexpected response — please retry.', tone: 'err' };
  const { result } = res;
  if (result.error) return { text: result.error, tone: 'err' };
  const title = result.title?.trim();
  if (result.partial) {
    const lead = title ? `Imported “${title}”` : 'Imported';
    return {
      text: `${lead} — couldn't read the description. ${IMPORT_PARTIAL_HINT}`,
      tone: 'ok',
    };
  }
  const scoreSuffix = matchScoreSuffix(result.matchScore);
  if (!requestedApplied && result.status && result.status !== 'saved') {
    const label = capitalize(result.status);
    const lead = title
      ? `“${title}” is already tracked as ${label}`
      : `This job is already tracked as ${label}`;
    return {
      text: `${lead} — status unchanged. ${IMPORT_LANDING_HINT}${scoreSuffix}`,
      tone: 'ok',
    };
  }
  const lead = title ? `Imported “${title}”.` : 'Imported.';
  return { text: `${lead} ${IMPORT_LANDING_HINT}${scoreSuffix}`, tone: 'ok' };
}

/** Default/found labels for the import button — adaptive per the applied.check
 *  outcome (same click action either way; the desktop always dedup-merges by
 *  url, so "re-import" is just the honest label when a row already exists). */
const IMPORT_LABEL_DEFAULT = 'Import this job';
const IMPORT_LABEL_FOUND = 'Re-import / update';

/** Capitalize a single-word lowercase id (e.g. an `ApplicationStatus` wire id)
 *  for display — reused by both the import transparency message and the
 *  applied-check status line. */
function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Format an epoch-ms timestamp as a short local date (e.g. "Jun 12", or
 *  "Jun 12, 2025" when the date's year differs from the current year) —
 *  popup-local formatting, no date library. */
function formatShortDate(epochMs: number): string {
  const date = new Date(epochMs);
  const opts: Intl.DateTimeFormatOptions = { month: 'short', day: 'numeric' };
  if (date.getFullYear() !== new Date().getFullYear()) opts.year = 'numeric';
  return date.toLocaleDateString(undefined, opts);
}

/**
 * Given an `appliedCheck` response, return the status line to render above the
 * import controls, or `null` when nothing should be shown — not found, or ANY
 * error (the check is a silent best-effort enhancement, never a blocker; see
 * `runAppliedCheck` in background.ts, which already folds every failure mode
 * into `result.found === false`).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAppliedStatusLine(res: PopupResponse): string | null {
  if (!res.ok || res.kind !== 'appliedCheck') return null;
  const { result } = res;
  if (result.error || !result.found) return null;

  const title = result.title?.trim();
  const lead = title ? `“${title}”` : null;
  if (!result.status || result.status === 'saved') {
    return lead ? `${lead} is saved in your pipeline.` : 'Saved in your pipeline.';
  }
  const when = typeof result.appliedAt === 'number' ? formatShortDate(result.appliedAt) : null;
  if (lead && when) return `${lead} is already in your pipeline — applied ${when}.`;
  if (lead) return `${lead} is already in your pipeline.`;
  if (when) return `Already in your pipeline — applied ${when}.`;
  return 'Already in your pipeline.';
}

/**
 * The import button's label: unchanged when no existing Application was found
 * for the active tab's url, {@link IMPORT_LABEL_FOUND} when one was. Any
 * non-found/error outcome (including one still in flight) keeps the default.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveImportButtonLabel(res: PopupResponse): string {
  if (res.ok && res.kind === 'appliedCheck' && !res.result.error && res.result.found) {
    return IMPORT_LABEL_FOUND;
  }
  return IMPORT_LABEL_DEFAULT;
}

/**
 * Whether the "Mark as applied" button should show: only for a found
 * Application whose status is EXPLICITLY `saved` — the ONLY status this
 * write's CAS precondition can ever transition FROM (the bridge's
 * `saved → applied` compare-and-set requires the current status to already
 * be `saved`; an absent/unknown status is not the same guarantee). Any other
 * status (already applied, mid-pipeline, missing, or not found/error) keeps
 * the button hidden; those cases use the existing "I already applied" import
 * checkbox, not this button.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveShowMarkAppliedButton(res: PopupResponse): boolean {
  if (!res.ok || res.kind !== 'appliedCheck') return false;
  const { result } = res;
  if (result.error || !result.found) return false;
  return result.status === 'saved';
}

/** Whether the Form group + the Answer-tools disclosure should each be
 *  shown — see {@link resolveFieldsProbeResponse}. */
export interface FieldsProbeView {
  showFormGroup: boolean;
  showAnswerTools: boolean;
}

/**
 * Given a `fieldsProbe` response, whether the Form group and the
 * Answer-tools disclosure should each be shown. Fails OPEN (`true` for both)
 * on a transport-level `ok:false` or an unexpected `kind` — mirrors the
 * background's own fail-open fold (`runFieldsProbe`) so a probe bug can
 * never hide either feature; only a CONFIRMED `false` signal hides one.
 *
 * The two booleans are NOT the same gate: `hasFormFields` is the union of
 * autofill-supported identity fields and answer-capturable non-identity
 * fields (so "Fill this form" still shows on an identity-only form, e.g.
 * name/email/phone), while `hasAnswerFields` is the narrower
 * answer-capturable-only signal the Answer-tools disclosure uses (Suggest/
 * rewrite have nothing to act on from identity fields alone) — see
 * `PopupResponse`'s `fieldsProbe` doc.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveFieldsProbeResponse(res: PopupResponse): FieldsProbeView {
  if (!res.ok || res.kind !== 'fieldsProbe') {
    return { showFormGroup: true, showAnswerTools: true };
  }
  return { showFormGroup: res.hasFormFields, showAnswerTools: res.hasAnswerFields };
}

/**
 * Given a `statusUpdate` response, return the message text + tone. UNLIKE
 * `resolveAppliedStatusLine`/`resolveImportButtonLabel` (which fold every
 * failure into "render nothing" — this is a passive, best-effort check),
 * this verb's errors ARE shown: it answers a deliberate click. A
 * transport-level `ok:false` surfaces its `error`; a resolved
 * `result.ok === false` (the desktop's own refusal — no match / wrong
 * starting status) surfaces `result.error`.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveMarkAppliedResponse(res: PopupResponse): {
  text: string;
  tone: 'ok' | 'err';
} {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'statusUpdate') {
    return { text: 'Unexpected response — please retry.', tone: 'err' };
  }
  const { result } = res;
  if (!result.ok) {
    return { text: result.error ?? 'Could not mark this job as applied.', tone: 'err' };
  }
  return { text: 'Marked as applied.', tone: 'ok' };
}

/**
 * Given an `answersSave` response, return the message text + tone. Mirrors
 * `resolveMarkAppliedResponse` — this verb's errors ARE shown (a deliberate
 * click, not a passive check). On success names the job from the reply's
 * `title`/`company` (the smaller change vs. threading the separately-fetched
 * `appliedCheck` state through this confirmation — see the PR-5 handoff) and
 * reports the saved count; a re-capture with nothing new to add reads as a
 * benign "no new answers", never an error. When the desktop dedupes/caps some
 * answers, `skipped` is folded into the copy too — `saved === 0` gets a
 * distinct "already recorded" message instead of the generic no-new-answers one.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAnswersSaveResponse(res: PopupResponse): {
  text: string;
  tone: 'ok' | 'err';
} {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'answersSave') {
    return { text: 'Unexpected response — please retry.', tone: 'err' };
  }
  const { result } = res;
  if (!result.ok) return { text: result.error, tone: 'err' };

  const title = result.title?.trim();
  const company = result.company?.trim();
  const name = title && company ? `${title} @ ${company}` : (title ?? company);

  if (result.saved === 0) {
    if (result.skipped > 0) {
      const was = result.skipped === 1 ? 'was' : 'were';
      const noun = `answer${result.skipped === 1 ? '' : 's'}`;
      return { text: `All ${result.skipped} ${noun} ${was} already recorded.`, tone: 'ok' };
    }
    return { text: 'No new answers to save from this page.', tone: 'ok' };
  }
  const count = `${result.saved} answer${result.saved === 1 ? '' : 's'}`;
  const base = name ? `Saved ${count} to ${name}` : `Saved ${count}`;
  const suffix = result.skipped > 0 ? ` — ${result.skipped} already recorded.` : '.';
  return { text: `${base}${suffix}`, tone: 'ok' };
}

/**
 * Pair each desktop-returned suggestion with its scan-time fill correlation.
 * When `scanned` contains EXACTLY ONE field sharing the suggestion's exact
 * question text, `fieldIndex` is `0` (that field's own occurrence index).
 * When `scanned` contains NONE or MORE THAN ONE such field, `fieldIndex` is
 * `null` and `multipleMatches` records which case it was — a page with two+
 * fields sharing the identical label is ambiguous (which one would "Fill"
 * even mean?), so it must never guess: Fill is omitted and the row falls back
 * to Copy-only, same fail-safe discipline as `locateQuestionField`'s re-scan.
 *
 * Pure: no DOM access, no side effects.
 */
export interface RenderedSuggestion {
  suggestion: ExtensionAnswerSuggestion;
  fieldIndex: number | null;
  /** `true` when the scan found MORE THAN ONE field sharing this exact
   *  question text — the row shows a "fill manually" hint instead of a
   *  (necessarily ambiguous) Fill button. */
  multipleMatches: boolean;
  /** Total live fields sharing this exact question text AT SCAN TIME (always
   *  `1` whenever `fieldIndex` is non-null). Sent alongside `fieldIndex` on a
   *  Fill click so the fill-time re-scan can refuse if the CURRENT count
   *  differs — a same-labelled field inserted earlier in DOM order between
   *  scan and click must never silently receive the fill. */
  scanCount: number;
}

export function correlateSuggestions(
  suggestions: ExtensionAnswerSuggestion[],
  scanned: ScannedQuestion[]
): RenderedSuggestion[] {
  return suggestions.map((suggestion) => {
    const matches = scanned.filter((q) => q.question === suggestion.question).length;
    return {
      suggestion,
      fieldIndex: matches === 1 ? 0 : null,
      multipleMatches: matches > 1,
      scanCount: matches,
    };
  });
}

/**
 * Given an `answersSuggest` response, return the message text + tone plus the
 * suggestions to render and the scan-time correlation list. Mirrors
 * `resolveAnswersSaveResponse` — this verb's errors ARE shown (a deliberate
 * click, not a passive check).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAnswersSuggestResponse(res: PopupResponse): {
  text: string;
  tone: 'ok' | 'err';
  suggestions: ExtensionAnswerSuggestion[];
  scanned: ScannedQuestion[];
} {
  if (!res.ok) return { text: res.error, tone: 'err', suggestions: [], scanned: [] };
  if (res.kind !== 'answersSuggest') {
    return {
      text: 'Unexpected response — please retry.',
      tone: 'err',
      suggestions: [],
      scanned: [],
    };
  }
  const { result, scanned } = res;
  if (!result.ok) return { text: result.error, tone: 'err', suggestions: [], scanned };
  if (result.suggestions.length === 0) {
    return {
      text: 'No matching past answers found for this form.',
      tone: 'ok',
      suggestions: [],
      scanned,
    };
  }
  const count = result.suggestions.length;
  return {
    text: `Found ${count} suggestion${count === 1 ? '' : 's'} for this form.`,
    tone: 'ok',
    suggestions: result.suggestions,
    scanned,
  };
}

/**
 * Given a `fill` response, return the popup message + tone. The detailed summary
 * lives in the in-page overlay; the popup shows a short confirmation (or the
 * desktop's refusal when autofill is opted out). Handles the "nothing matched"
 * case explicitly so a no-op never reads as a failure.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveFillResponse(res: PopupResponse): { text: string; tone: 'ok' | 'err' } {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'fill') return { text: 'Unexpected response — please retry.', tone: 'err' };
  const { summary } = res;
  if (summary.filledNothing) {
    return { text: 'No matchable fields found on this page.', tone: 'ok' };
  }
  const total = summary.filled.reduce((n, f) => n + f.count, 0);
  const base = `Filled ${total} field${total === 1 ? '' : 's'} — review them on the page`;
  return {
    text: summary.nameSplit ? `${base} (name split is a guess — verify).` : `${base}.`,
    tone: 'ok',
  };
}

/** Human-readable label for `scoreSource` — `'combined'` is wire-reserved and
 *  never sent by the current desktop (keyword-only always), but the label
 *  exists so a future desktop's value renders sensibly without a popup change. */
const SCORE_SOURCE_LABEL: Record<'keyword' | 'combined', string> = {
  keyword: 'keyword coverage',
  combined: 'combined (keyword + semantic)',
};

/** The "Check fit" score to render, or `null` fields when there is nothing to show. */
export interface MatchLiveView {
  text: string;
  tone: 'ok' | 'err';
  score: number | null;
  scoreLabel: string | null;
  resumeName: string | null;
  gaps: string[];
}

const NO_MATCH_VIEW = (text: string, tone: 'ok' | 'err'): MatchLiveView => ({
  text,
  tone,
  score: null,
  scoreLabel: null,
  resumeName: null,
  gaps: [],
});

/**
 * Given a `matchLive` response, return the message text + tone plus the score
 * to render (percent, source label, résumé name, missing-keyword gaps).
 * Mirrors `resolveAnswersSuggestResponse` — this verb's errors ARE shown (a
 * deliberate click, not a passive check).
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveMatchLiveResponse(res: PopupResponse): MatchLiveView {
  if (!res.ok) return NO_MATCH_VIEW(res.error, 'err');
  if (res.kind !== 'matchLive') {
    return NO_MATCH_VIEW('Unexpected response — please retry.', 'err');
  }
  const { result } = res;
  if (!result.ok) return NO_MATCH_VIEW(result.error, 'err');

  const score = Math.round(result.combined);
  return {
    text: `${score}% fit against “${result.resumeName}”.`,
    tone: 'ok',
    score,
    scoreLabel: SCORE_SOURCE_LABEL[result.scoreSource],
    resumeName: result.resumeName,
    gaps: result.gaps,
  };
}

/**
 * Given an `answerAssist` response, return the message text + tone plus the
 * draft to render (`null` when there is nothing to show — an error).
 * Mirrors `resolveMatchLiveResponse` — this verb's errors ARE shown (a
 * deliberate click, not a passive check).
 *
 * Pure: no DOM access, no side effects.
 */
export interface AnswerAssistView {
  text: string;
  tone: 'ok' | 'err';
  draft: string | null;
}

export function resolveAnswerAssistResponse(res: PopupResponse): AnswerAssistView {
  if (!res.ok) return { text: res.error, tone: 'err', draft: null };
  if (res.kind !== 'answerAssist') {
    return { text: 'Unexpected response — please retry.', tone: 'err', draft: null };
  }
  const { result } = res;
  if (!result.ok) return { text: result.error, tone: 'err', draft: null };
  return { text: 'Draft ready — review before using it.', tone: 'ok', draft: result.draft };
}

/**
 * Given the background's current/last streamed `answer.assist` snapshot
 * (`{text, done, interrupted}` — see `PopupResponse`'s `answerAssistProgress`
 * doc), return what the popup should render. Used for BOTH the live push
 * while a stream is running and the popup-open reattach query.
 * `draft === null` means there is nothing to show at all (no stream has run
 * this session) — the caller should leave the draft box untouched.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAssistProgressView(progress: {
  text: string;
  done: boolean;
  interrupted: boolean;
}): AnswerAssistView {
  if (!progress.text) return { text: '', tone: 'ok', draft: null };
  if (progress.interrupted) {
    return {
      text: 'Connection interrupted — here is what arrived so far.',
      tone: 'err',
      draft: progress.text,
    };
  }
  if (!progress.done) {
    return { text: 'Drafting an answer…', tone: 'ok', draft: progress.text };
  }
  return { text: 'Draft ready — review before using it.', tone: 'ok', draft: progress.text };
}

/**
 * Populate the "pick a scanned question" `<select>` from the most recent
 * questions-mode scan (deduped by exact text, in scan order). Pure DOM
 * projection so it's straightforward to re-derive whenever
 * `lastScannedQuestions` changes — no separate scan injection for this
 * feature, it reuses whatever "Suggest answers for this form" last scanned.
 *
 * Pure: no side effects beyond the returned option list (the caller writes it
 * into the DOM).
 */
export function buildAssistPickerOptions(scanned: { question: string }[]): string[] {
  return [...new Set(scanned.map((q) => q.question).filter((q) => q.trim().length > 0))];
}

function byId<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el as T;
}

const els = {
  pill: byId<HTMLSpanElement>('status-pill'),
  views: {
    import: byId<HTMLElement>('view-import'),
    pair: byId<HTMLElement>('view-pair'),
    offline: byId<HTMLElement>('view-offline'),
    outdated: byId<HTMLElement>('view-outdated'),
    searching: byId<HTMLElement>('view-searching'),
  },
  btnImport: byId<HTMLButtonElement>('btn-import'),
  btnFill: byId<HTMLButtonElement>('btn-fill'),
  btnMarkApplied: byId<HTMLButtonElement>('btn-mark-applied'),
  groupForm: byId<HTMLElement>('group-form'),
  answerTools: byId<HTMLDetailsElement>('answer-tools'),
  btnSaveAnswers: byId<HTMLButtonElement>('btn-save-answers'),
  btnSuggestAnswers: byId<HTMLButtonElement>('btn-suggest-answers'),
  suggestionsList: byId<HTMLDivElement>('suggestions-list'),
  btnCheckFit: byId<HTMLButtonElement>('btn-check-fit'),
  matchResult: byId<HTMLDivElement>('match-result'),
  assistPicker: byId<HTMLSelectElement>('assist-picker'),
  assistQuestion: byId<HTMLTextAreaElement>('assist-question'),
  chkSearchWeb: byId<HTMLInputElement>('chk-search-web'),
  btnAssist: byId<HTMLButtonElement>('btn-assist'),
  assistResult: byId<HTMLDivElement>('assist-result'),
  assistDraft: byId<HTMLParagraphElement>('assist-draft'),
  btnCopyAssist: byId<HTMLButtonElement>('btn-copy-assist'),
  rewritePicker: byId<HTMLSelectElement>('rewrite-picker'),
  rewritePreset: byId<HTMLSelectElement>('rewrite-preset'),
  rewriteInstruction: byId<HTMLInputElement>('rewrite-instruction'),
  btnRewrite: byId<HTMLButtonElement>('btn-rewrite'),
  rewriteResult: byId<HTMLDivElement>('rewrite-result'),
  rewriteDraft: byId<HTMLParagraphElement>('rewrite-draft'),
  btnCopyRewrite: byId<HTMLButtonElement>('btn-copy-rewrite'),
  btnAcceptRewrite: byId<HTMLButtonElement>('btn-accept-rewrite'),
  btnRestoreRewrite: byId<HTMLButtonElement>('btn-restore-rewrite'),
  appliedStatus: byId<HTMLParagraphElement>('applied-status'),
  chkApplied: byId<HTMLInputElement>('chk-applied'),
  importMsg: byId<HTMLParagraphElement>('import-msg'),
  unpairGroup: byId<HTMLElement>('unpair-group'),
  btnUnpair: byId<HTMLButtonElement>('btn-unpair'),
  tokenInput: byId<HTMLInputElement>('token-input'),
  pairMsg: byId<HTMLParagraphElement>('pair-msg'),
  btnSaveToken: byId<HTMLButtonElement>('btn-save-token'),
  btnRetry: byId<HTMLButtonElement>('btn-retry'),
  btnOpenSettings: byId<HTMLButtonElement>('btn-open-settings'),
  btnHelp: byId<HTMLButtonElement>('btn-help'),
  helpPopover: byId<HTMLParagraphElement>('help-popover'),
  btnGetApp: byId<HTMLButtonElement>('btn-get-app'),
  btnUpdateApp: byId<HTMLButtonElement>('btn-update-app'),
};

/** Actionable label for the pairing button; restored after a failed/cleared pair. */
const PAIR_LABEL = 'Save & pair';

/** How long the "✓ Authorized" confirmation stays on the pair button before the
 *  popup flips to the import view, so the success state is actually seen. */
const AUTHORIZED_CONFIRM_MS = 800;

const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Pill labels carry a non-color glyph prefix so the connection state is
 * distinguishable without relying on color alone (deuteranopia-safe).
 */
const PILL_LABEL: Record<ConnectionStatus['phase'], string> = {
  searching: '○ Connecting…',
  not_paired: '⚠ Not paired',
  connected: '● Connected',
  app_not_running: '✕ App not running',
  outdated: '⟳ Update the app',
  bad_token: '✕ Wrong token',
};

/** First status resolves within this budget, else fall back to the offline/Retry view. */
const STATUS_TIMEOUT_MS = 3_000;

/** Desktop deep link: launches/focuses the app on Settings → Browser extension
 *  with the pairing token highlighted. The click is the required user gesture;
 *  the browser may show its own "Open AI Job Hunter?" confirmation (expected). */
const PAIRING_DEEP_LINK = 'ajh://settings/extension';

/** Public download page, offered in the offline view for users who don't yet
 *  have the desktop app installed. */
const GET_APP_URL = 'https://aijobhunter.app/download';

/**
 * Last-known token state, cached so a transient `!ok` status reply (asleep or
 * just-woken service worker, message-channel race) can render the offline view
 * without spuriously telling a paired user to re-pair.
 */
let lastKnownHasToken = false;

/**
 * Set to `true` once the offline (`app_not_running`) view has been shown.
 * While `true`, a transient `searching` status from a background reconnect
 * attempt does NOT swap out the offline guidance — the user already knows the
 * app is unreachable; briefly hiding the "Get the app" content on every retry
 * cycle is disorienting. Reset to `false` when a real outcome arrives
 * (`connected`, `not_paired`, or `bad_token`).
 */
let hasShownOffline = false;

/**
 * The phase from the previous `render()` call. Used to fire the
 * fire-and-forget `appliedCheck` auto-check exactly once per TRANSITION into
 * `connected` — not on every status push while already connected (a repeated
 * live-status push during a stable connection must not re-fire it), but a
 * genuine reconnect after a drop naturally re-checks, since that is a fresh
 * transition too.
 */
let lastRenderedPhase: ConnectionStatus['phase'] | null = null;

/**
 * The most recent questions-mode scan (`{question, index}[]`), kept so the
 * currently-rendered suggestion rows can correlate each suggestion to a live
 * fill target — see `correlateSuggestions`. Cleared whenever the popup
 * leaves the `connected` view (see `render`) so a stale correlation can
 * never survive into a different page.
 */
let lastScannedQuestions: ScannedQuestion[] = [];

/**
 * The most recent filled-fields scan (`{question, index, answer}[]`, PR 11)
 * — the rewrite picker's option list. Populated by the SAME "Save my answers
 * from this page" scan `answersSave` already runs (see `capture.ts`), no
 * separate injection. Cleared on leaving `connected` — same discipline as
 * {@link lastScannedQuestions}.
 */
let lastScannedFilled: FilledField[] = [];

/**
 * The currently-picked rewrite target — the field's scan-time correlation
 * (`question`/`index`/`expectedCount`, mirroring `answerFill`'s own
 * correlation shape), the FROZEN original text at pick time (what "Restore
 * original" re-injects, NEVER updated), and `expectedValue` — what THIS
 * popup instance believes the field currently holds, sent on every
 * Accept/Restore so `replaceFilledField` can refuse (never clobber) a
 * manual edit made since. Starts equal to `originalAnswer` and is updated to
 * whatever text a successful Accept/Restore just wrote, so the NEXT
 * Accept/Restore compares against the right baseline — see
 * `sendRewriteReplace`. `null` until the picker selects a field; reset
 * whenever the picker changes or a fresh scan re-renders it.
 */
let rewriteTarget: {
  question: string;
  index: number;
  expectedCount: number;
  originalAnswer: string;
  expectedValue: string;
} | null = null;

/**
 * Which draft box the CURRENT (or most recently started, in THIS popup
 * instance) `answer.assist` stream feeds — draft's `assistDraft` or
 * rewrite's `rewriteDraft`. Both modes share the SAME background-owned
 * streaming buffer/generation guard (see `background.ts`'s `assistBuffer`
 * doc) — this local flag is only about which of the two boxes a live
 * `answerAssistProgress` push (or the popup-open reattach) should update; it
 * does not affect which request is actually in flight. Set right before
 * `doAssist`/`doRewrite` sends its request. Defaults to `'draft'` — a popup
 * reopened mid-stream (this flag reset to its default by the fresh
 * instance) falls back to the draft box, a documented, minor limitation
 * (PR 11 does not thread mode through the reattach path).
 */
let activeAssistKind: 'draft' | 'rewrite' = 'draft';

/** Send a typed request to the background and return its typed response. */
async function send(req: PopupRequest): Promise<PopupResponse> {
  const res = (await browser.runtime.sendMessage(req)) as PopupResponse | undefined;
  if (!res) return { ok: false, error: 'No response from the extension background.' };
  return res;
}

/** Toggle the Form group + Answer-tools disclosure — each on its OWN signal
 *  (see {@link FieldsProbeView}'s doc for why they differ), driven by the
 *  fields probe (`runFieldsProbeCheck`). */
function setToolGroupsVisible(view: FieldsProbeView): void {
  els.groupForm.hidden = !view.showFormGroup;
  els.answerTools.hidden = !view.showAnswerTools;
}

function showView(phase: ConnectionStatus['phase']): void {
  els.views.import.hidden = phase !== 'connected';
  // Show the pairing view for both not_paired and bad_token — the user must
  // enter a corrected token in both cases.
  els.views.pair.hidden = phase !== 'not_paired' && phase !== 'bad_token';
  els.views.offline.hidden = phase !== 'app_not_running';
  // Outdated desktop: a distinct "update the desktop app" view (NOT the pairing
  // view — the token is fine; the app is too old to speak the v2 handshake).
  els.views.outdated.hidden = phase !== 'outdated';
  els.views.searching.hidden = phase !== 'searching';
}

function render(status: ConnectionStatus): void {
  lastKnownHasToken = status.hasToken;
  // The help popover is global (not scoped to any one view/phase) — only show
  // "Unpair this device" while there is actually something to unpair.
  els.unpairGroup.hidden = !status.hasToken;

  if (status.phase === 'connected') {
    // Fire-and-forget, on each transition INTO connected (never on a repeated
    // push while already connected) — never awaited here, so it can never delay
    // this render.
    if (lastRenderedPhase !== 'connected') {
      void runAppliedAutoCheck();
      void runFieldsProbeCheck();
    }
  } else {
    // Left (or never entered) `connected` — clear any status line/button label
    // left over from a previous page so it can't flash stale for the next one
    // before its own check resolves.
    els.appliedStatus.hidden = true;
    els.appliedStatus.textContent = '';
    els.btnImport.textContent = IMPORT_LABEL_DEFAULT;
    els.btnMarkApplied.hidden = true;
    els.btnMarkApplied.disabled = false;
    // A stale suggestion list (and its fill correlation) must never linger
    // into a different page's connected view.
    els.suggestionsList.hidden = true;
    els.suggestionsList.textContent = '';
    lastScannedQuestions = [];
    // A stale "Check fit" score from a previous page must never linger either.
    els.matchResult.hidden = true;
    els.matchResult.textContent = '';
    // A stale AI-answer draft (and the picker it was scanned against) must
    // never linger into a different page's connected view either.
    els.assistResult.hidden = true;
    els.assistDraft.textContent = '';
    els.assistQuestion.value = '';
    els.chkSearchWeb.checked = false;
    renderAssistPicker([]);
    // A stale rewrite draft/target must never linger either (PR 11).
    renderRewritePicker([]);
    els.rewriteResult.hidden = true;
    els.rewriteDraft.textContent = '';
    els.rewriteInstruction.value = '';
    els.rewritePreset.value = '';
    // Invalidate any in-flight fieldsProbe BEFORE resetting visibility: a
    // pending `hasFields:false` from the page just left could otherwise
    // resolve AFTER this reset and hide the groups again in a phase that was
    // never gated on it (e.g. after a disconnect) — bumping the generation
    // makes runFieldsProbeCheck's own resolved-response branch a no-op.
    fieldsProbeGeneration += 1;
    // A stale "no fields on the previous page" hide must never linger onto a
    // fresh page before its own probe resolves — default open/visible again.
    setToolGroupsVisible({ showFormGroup: true, showAnswerTools: true });
  }
  lastRenderedPhase = status.phase;

  // Track whether the offline view has been shown so we can suppress the
  // flickering "Connecting…" spinner during background reconnect attempts.
  if (status.phase === 'app_not_running') {
    hasShownOffline = true;
  } else if (
    status.phase === 'connected' ||
    status.phase === 'not_paired' ||
    status.phase === 'bad_token' ||
    status.phase === 'outdated'
  ) {
    // A real outcome arrived — reset so the next session starts fresh.
    hasShownOffline = false;
  }

  // If a transient reconnect attempt (`searching`) arrives AFTER the offline
  // view was shown, keep the offline guidance visible. Only update the pill
  // label and keep the Retry button so the user can see the retry is happening,
  // but do NOT swap the view — that would hide the "Get the app" content.
  if (status.phase === 'searching' && hasShownOffline) {
    els.pill.textContent = PILL_LABEL.searching;
    els.pill.className = `pill pill--searching`;
    els.btnRetry.hidden = false;
    return;
  }

  els.pill.textContent = PILL_LABEL[status.phase];
  els.pill.className = `pill pill--${status.phase}`;
  // Retry lives in the header (left of the pill) and makes sense when the app is
  // unreachable OR outdated (re-probe after the user updates the desktop app).
  els.btnRetry.hidden = status.phase !== 'app_not_running' && status.phase !== 'outdated';
  showView(status.phase);
  // On bad_token, surface a clear error in the pairing view so the user knows
  // they need to copy the current token from the desktop app's Settings.
  if (status.phase === 'bad_token') {
    setMsg(
      els.pairMsg,
      "That token didn't match — copy the current token from the desktop app's Settings and try again.",
      'err'
    );
  } else if (status.phase === 'not_paired') {
    els.pairMsg.textContent = '';
  }
}

/**
 * Render the offline / Retry view without a fresh status from the background.
 * Used when the background is unreachable (transient `!ok`) or the first
 * status request times out, so the popup never stays stuck on the spinner.
 */
function renderOffline(): void {
  render({ phase: 'app_not_running', port: null, hasToken: lastKnownHasToken });
}

function setMsg(el: HTMLElement, text: string, tone: 'ok' | 'err' | 'muted'): void {
  el.textContent = text;
  el.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
}

async function refreshStatus(): Promise<void> {
  const res = await send({ kind: 'getStatus' });
  // resolveStatusResponse always returns a ConnectionStatus — offline fallback
  // when the background is unreachable; `!ok` path yields app_not_running.
  render(resolveStatusResponse(res, lastKnownHasToken));
}

/**
 * First status fetch with a timeout backstop: if the background does not answer
 * within {@link STATUS_TIMEOUT_MS}, fall back to the offline/Retry view rather
 * than spin indefinitely. A later status push or Retry will recover.
 */
async function refreshStatusWithTimeout(): Promise<void> {
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    renderOffline();
  }, STATUS_TIMEOUT_MS);
  try {
    const res = await send({ kind: 'getStatus' });
    if (timedOut) return;
    render(resolveStatusResponse(res, lastKnownHasToken));
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Poll `getStatus` until the phase leaves `searching` (the bridge connection has
 * settled to connected / not_paired / app_not_running) or the attempts run out.
 * Renders each result. A safety net so the popup never strands on the "searching"
 * spinner when the background's live status push is missed (MV3 race / just-woken
 * worker). The live `onMessage` push still updates the view independently.
 */
async function refreshUntilSettled(attempts = 5, gapMs = 600): Promise<void> {
  for (let i = 0; i < attempts; i += 1) {
    try {
      const res = await send({ kind: 'getStatus' });
      const status = resolveStatusResponse(res, lastKnownHasToken);
      render(status);
      if (status.phase !== 'searching') return;
    } catch {
      // A transient MV3 message-channel rejection here must NOT bubble into the
      // savePairing catch (a false "Pairing failed" after a successful pair). The
      // live status push / next popup open recovers; show offline and stop.
      renderOffline();
      return;
    }
    if (i < attempts - 1) await delay(gapMs);
  }
}

/**
 * Generation counter guarding {@link runAppliedAutoCheck} against a stale
 * in-flight response. A disconnect→reconnect re-enters `connected` and fires
 * a fresh check while the previous one may still be awaiting `send()`; if the
 * stale one resolves (or rejects) AFTER the newer check has started, it must
 * not overwrite the newer result.
 */
let appliedCheckGeneration = 0;

/**
 * Run the fire-and-forget `appliedCheck` and render its outcome: the status
 * line above the import controls, plus the adaptive import-button label.
 * `runAppliedCheck` in background.ts already folds every failure mode into
 * `ok:true, result:{found:false}`, so the try/catch here only guards a
 * transport-level rejection (message-channel closed) — either way nothing is
 * ever shown but "no line, default label".
 */
async function runAppliedAutoCheck(): Promise<void> {
  appliedCheckGeneration += 1;
  const myGeneration = appliedCheckGeneration;
  // Clear synchronously before the request goes out (belt-and-suspenders): if
  // render() re-enters `connected` for a new page while a previous check is
  // still in flight, the previous page's line/label must not linger while
  // this fresh one resolves.
  els.appliedStatus.hidden = true;
  els.appliedStatus.textContent = '';
  els.btnImport.textContent = IMPORT_LABEL_DEFAULT;
  els.btnMarkApplied.hidden = true;
  els.btnMarkApplied.disabled = false;
  try {
    const res = await send({ kind: 'appliedCheck' });
    // A newer check started while this one was in flight — its result (or the
    // DOM state the newer check already wrote) must win; bail before touching
    // the DOM.
    if (myGeneration !== appliedCheckGeneration) return;
    const line = resolveAppliedStatusLine(res);
    els.appliedStatus.hidden = line === null;
    els.appliedStatus.textContent = line ?? '';
    els.btnImport.textContent = resolveImportButtonLabel(res);
    // Only a found+saved result shows the button — reset disabled here too,
    // so a re-fire after a successful "Mark as applied" click (which left the
    // button disabled) ends re-enabled for whatever this fresh check renders.
    els.btnMarkApplied.hidden = !resolveShowMarkAppliedButton(res);
    els.btnMarkApplied.disabled = false;
  } catch {
    if (myGeneration !== appliedCheckGeneration) return;
    els.appliedStatus.hidden = true;
    els.appliedStatus.textContent = '';
    els.btnImport.textContent = IMPORT_LABEL_DEFAULT;
    els.btnMarkApplied.hidden = true;
    els.btnMarkApplied.disabled = false;
  }
}

/**
 * Generation guard for {@link runFieldsProbeCheck} — mirrors
 * `appliedCheckGeneration` exactly (same stale-response race the applied
 * auto-check already guards against).
 */
let fieldsProbeGeneration = 0;

/**
 * Run the fire-and-forget "does this page have fillable form fields?" probe
 * on entering `connected` and gate the Form group + Answer-tools disclosure
 * on the result (each on its own signal — see {@link resolveFieldsProbeResponse}).
 * `runFieldsProbe` in background.ts already folds every failure (no active
 * tab, restricted page, scripting denied) into both signals `true` (fail
 * OPEN), so the catch here only guards a transport-level rejection
 * (message-channel closed) — either way this never hides a group on a probe
 * bug, only on a confirmed empty scan.
 */
async function runFieldsProbeCheck(): Promise<void> {
  fieldsProbeGeneration += 1;
  const myGeneration = fieldsProbeGeneration;
  try {
    const res = await send({ kind: 'fieldsProbe' });
    if (myGeneration !== fieldsProbeGeneration) return;
    setToolGroupsVisible(resolveFieldsProbeResponse(res));
  } catch {
    if (myGeneration !== fieldsProbeGeneration) return;
    setToolGroupsVisible({ showFormGroup: true, showAnswerTools: true });
  }
}

/**
 * Click handler for "Mark as applied". Sends `status.update` and shows the
 * result in the existing message area — UNLIKE the passive auto-check,
 * failures ARE shown here (this is a deliberate click action). On success it
 * re-fires {@link runAppliedAutoCheck} (the SAME generation-guarded path
 * every other applied.check render goes through) instead of hand-rolling a
 * DOM update, so the status line flips to the applied wording and this
 * button hides itself once the fresh check confirms it.
 */
async function doMarkApplied(): Promise<void> {
  els.btnMarkApplied.disabled = true;
  setMsg(els.importMsg, 'Marking as applied…', 'muted');
  try {
    const res = await send({ kind: 'statusUpdate' });
    const { text, tone } = resolveMarkAppliedResponse(res);
    setMsg(els.importMsg, text, tone);
    if (tone === 'ok') {
      void runAppliedAutoCheck();
    } else {
      els.btnMarkApplied.disabled = false;
    }
  } catch {
    // A transport/messaging rejection must not strand the button disabled.
    setMsg(els.importMsg, 'Could not mark this job as applied. Please retry.', 'err');
    els.btnMarkApplied.disabled = false;
  }
}

/**
 * Click handler for "Save my answers from this page". Sends `answersSave`
 * and shows the result in the existing message area — UNLIKE the passive
 * auto-check, failures ARE shown here (this is a deliberate click action).
 * Also renders the rewrite picker (PR 11) from the SAME response's `filled`
 * scan — no separate scan injection for that feature, mirroring how
 * `doSuggestAnswers` feeds the draft-mode picker from its own scan.
 */
async function doSaveAnswers(): Promise<void> {
  els.btnSaveAnswers.disabled = true;
  setMsg(els.importMsg, 'Saving your answers…', 'muted');
  try {
    const res = await send({ kind: 'answersSave' });
    const { text, tone } = resolveAnswersSaveResponse(res);
    setMsg(els.importMsg, text, tone);
    renderRewritePicker(res.ok && res.kind === 'answersSave' ? res.filled : []);
  } catch {
    // A transport/messaging rejection must not strand the status on "Saving…".
    setMsg(els.importMsg, 'Could not save your answers. Please retry.', 'err');
  } finally {
    els.btnSaveAnswers.disabled = false;
  }
}

/** Truncate an answer preview for the suggestion row (never the full text —
 *  this is a preview, the full text travels to Copy/Fill unchanged). */
function truncateAnswer(s: string, max = 140): string {
  const t = s.trim();
  return t.length > max ? `${t.slice(0, max - 1)}…` : t;
}

/** Copy `text` to the clipboard; returns whether it succeeded. Extension
 *  pages may call `navigator.clipboard.writeText` on a user gesture without
 *  an extra permission. */
async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Click handler for a suggestion row's Copy button — briefly confirms on
 *  the button itself, never touches the shared message area. */
async function doCopySuggestion(text: string, btn: HTMLButtonElement): Promise<void> {
  const original = btn.textContent;
  const ok = await copyText(text);
  btn.textContent = ok ? '✓ Copied' : 'Copy failed';
  setTimeout(() => {
    btn.textContent = original;
  }, 1200);
}

/**
 * Click handler for a suggestion row's "Fill this field" button. `question`/
 * `fieldIndex`/`expectedCount` are the SAME scan-time correlation the
 * collector produced — the filler re-locates that exact field and fails safe
 * (never a different field) if the page changed since the scan, INCLUDING a
 * same-labelled field inserted/removed elsewhere on the page (a count change,
 * not just an out-of-range index).
 */
async function doFillSuggestion(
  question: string,
  fieldIndex: number,
  expectedCount: number,
  answer: string,
  btn: HTMLButtonElement
): Promise<void> {
  btn.disabled = true;
  const original = btn.textContent;
  try {
    const res = await send({
      kind: 'answerFill',
      question,
      index: fieldIndex,
      count: expectedCount,
      answer,
    });
    if (res.ok && res.kind === 'answerFill' && res.result.filled) {
      btn.textContent = '✓ Filled';
      return;
    }
    const text =
      res.ok && res.kind === 'answerFill'
        ? (res.result.error ?? 'Could not fill this field.')
        : !res.ok
          ? res.error
          : 'Could not fill this field.';
    setMsg(els.importMsg, text, 'err');
    btn.disabled = false;
    btn.textContent = original;
  } catch {
    setMsg(els.importMsg, 'Could not fill this field. Please retry.', 'err');
    btn.disabled = false;
    btn.textContent = original;
  }
}

/** Build one suggestion row: question / answer preview / source (always
 *  including the matched candidate's original `sourceQuestion`, so a
 *  cross-question match is visible, not silent), plus Copy and (when a live
 *  target exists and the question is not salary-like) Fill. When the scan
 *  found more than one field sharing this exact question — which one to fill
 *  is ambiguous — a short hint replaces Fill instead of guessing.
 *  `textContent` only — no `innerHTML` with page/desktop-derived text. */
function buildSuggestionRow(item: RenderedSuggestion): HTMLElement {
  const { suggestion, fieldIndex, multipleMatches, scanCount } = item;
  const row = document.createElement('div');
  row.className = 'suggestion';

  const q = document.createElement('p');
  q.className = 'suggestion__question';
  q.textContent = suggestion.question;
  row.append(q);

  const a = document.createElement('p');
  a.className = 'suggestion__answer';
  a.textContent = truncateAnswer(suggestion.answer);
  row.append(a);

  const sourceTitle = suggestion.sourceTitle?.trim();
  const sourceCompany = suggestion.sourceCompany?.trim();
  const sourceQuestion = suggestion.sourceQuestion.trim();
  if (sourceTitle || sourceCompany || sourceQuestion) {
    const name =
      sourceTitle && sourceCompany
        ? `${sourceTitle} @ ${sourceCompany}`
        : (sourceTitle ?? sourceCompany);
    const src = document.createElement('p');
    src.className = 'suggestion__source';
    // Always shows the matched candidate's ORIGINAL question text — makes a
    // cross-question match (two questions similar enough on filler words to
    // score above the matcher's threshold but about different things)
    // self-evident rather than silent, whether or not it names an
    // application below.
    const bits = [`answered as: "${sourceQuestion}"`];
    if (name) bits.push(`from your ${name} application`);
    src.textContent = bits.join(' — ');
    row.append(src);
  }

  const actions = document.createElement('div');
  actions.className = 'suggestion__actions';

  const copyBtn = document.createElement('button');
  copyBtn.type = 'button';
  copyBtn.className = 'btn btn--small btn--quiet';
  copyBtn.textContent = 'Copy';
  copyBtn.addEventListener('click', () => void doCopySuggestion(suggestion.answer, copyBtn));
  actions.append(copyBtn);

  // Salary-like questions are Copy-only (never fillable), and Fill is only
  // offered when the scan found a live target for this exact question.
  if (!suggestion.salary && fieldIndex !== null) {
    const fillBtn = document.createElement('button');
    fillBtn.type = 'button';
    fillBtn.className = 'btn btn--small btn--quiet';
    fillBtn.textContent = 'Fill this field';
    fillBtn.addEventListener(
      'click',
      () =>
        void doFillSuggestion(
          suggestion.question,
          fieldIndex,
          scanCount,
          suggestion.answer,
          fillBtn
        )
    );
    actions.append(fillBtn);
  } else if (!suggestion.salary && multipleMatches) {
    // Ambiguous: more than one live field shares this exact label, so there is
    // no single field to target — never guess, tell the user to fill by hand.
    const hint = document.createElement('p');
    hint.className = 'suggestion__hint';
    hint.textContent = 'Multiple matching fields — fill manually.';
    actions.append(hint);
  }

  row.append(actions);
  return row;
}

/** Render the suggestion list — clears any prior rows first (no stale DOM
 *  from a previous scan). */
function renderSuggestions(suggestions: ExtensionAnswerSuggestion[]): void {
  els.suggestionsList.textContent = '';
  const rows = correlateSuggestions(suggestions, lastScannedQuestions);
  for (const item of rows) {
    els.suggestionsList.append(buildSuggestionRow(item));
  }
  els.suggestionsList.hidden = rows.length === 0;
}

/**
 * Click handler for "Suggest answers for this form". Scans the active tab's
 * empty candidate fields, sends their labels as `answers.suggest`, and
 * renders the returned suggestions. Errors ARE shown (a deliberate click).
 */
async function doSuggestAnswers(): Promise<void> {
  els.btnSuggestAnswers.disabled = true;
  els.suggestionsList.hidden = true;
  els.suggestionsList.textContent = '';
  setMsg(els.importMsg, 'Looking for matching answers…', 'muted');
  try {
    const res = await send({ kind: 'answersSuggest' });
    const { text, tone, suggestions, scanned } = resolveAnswersSuggestResponse(res);
    lastScannedQuestions = scanned;
    setMsg(els.importMsg, text, tone);
    renderSuggestions(suggestions);
    // "Help me answer…"'s picker reuses this SAME scan — no separate
    // injection for that feature.
    renderAssistPicker(scanned);
  } catch {
    // A transport/messaging rejection must not strand the status on "Looking…".
    setMsg(els.importMsg, 'Could not suggest answers for this page. Please retry.', 'err');
  } finally {
    els.btnSuggestAnswers.disabled = false;
  }
}

/** Rebuild the "pick a scanned question" `<select>` options from the most
 *  recent questions-mode scan — clears any prior options first (no stale
 *  entries from a previous page/scan). A change back to the picker's
 *  placeholder value is a no-op (the textarea is left as the user typed it). */
function renderAssistPicker(scanned: { question: string }[]): void {
  const placeholder = els.assistPicker.options[0];
  els.assistPicker.textContent = '';
  if (placeholder) els.assistPicker.append(placeholder);
  for (const question of buildAssistPickerOptions(scanned)) {
    const opt = document.createElement('option');
    opt.value = question;
    opt.textContent = question;
    els.assistPicker.append(opt);
  }
  els.assistPicker.value = '';
}

/**
 * Click handler for "Help me answer…" — the first BILLABLE-AI verb on the
 * bridge. Sends the textarea's (trimmed) text plus the "Search the web"
 * toggle. Errors ARE shown (a deliberate click, not a passive check) —
 * mirrors `doCheckFit`.
 */
async function doAssist(): Promise<void> {
  const question = els.assistQuestion.value.trim();
  if (!question) {
    setMsg(els.importMsg, 'Type or pick a question first.', 'err');
    return;
  }
  activeAssistKind = 'draft';
  els.btnAssist.disabled = true;
  els.assistResult.hidden = true;
  els.assistDraft.textContent = '';
  setMsg(els.importMsg, 'Drafting an answer…', 'muted');
  try {
    const res = await send({ kind: 'answerAssist', question, searchWeb: els.chkSearchWeb.checked });
    const view = resolveAnswerAssistResponse(res);
    setMsg(els.importMsg, view.text, view.tone);
    if (view.draft !== null) {
      // textContent only — this is AI-generated text, never rendered as HTML.
      els.assistDraft.textContent = view.draft;
      els.assistResult.hidden = false;
    }
  } catch {
    // A transport/messaging rejection must not strand the status on "Drafting…".
    setMsg(els.importMsg, 'Could not draft an answer. Please retry.', 'err');
  } finally {
    els.btnAssist.disabled = false;
  }
}

/** Click handler for the draft's Copy button — mirrors `doCopySuggestion`
 *  (briefly confirms on the button itself, never touches the shared message
 *  area). Copy-only: there is no fill path for AI-generated text. */
async function doCopyAssistDraft(): Promise<void> {
  const original = els.btnCopyAssist.textContent;
  const ok = await copyText(els.assistDraft.textContent ?? '');
  els.btnCopyAssist.textContent = ok ? '✓ Copied' : 'Copy failed';
  setTimeout(() => {
    els.btnCopyAssist.textContent = original;
  }, 1200);
}

// ── Rewrite mode (extension PR 11) ──────────────────────────────────────────

/** Rebuild the "pick a filled answer" `<select>` options from the most
 *  recent filled-fields scan (see `lastScannedFilled`) — clears any prior
 *  options first (no stale entries from a previous page/scan). Options are
 *  keyed by array index (not question text — the same question can appear
 *  more than once) and labelled with an occurrence suffix when a question
 *  repeats. Mirrors `renderAssistPicker`. */
function renderRewritePicker(filled: FilledField[]): void {
  lastScannedFilled = filled;
  const placeholder = els.rewritePicker.options[0];
  els.rewritePicker.textContent = '';
  if (placeholder) els.rewritePicker.append(placeholder);
  filled.forEach((f, i) => {
    const opt = document.createElement('option');
    opt.value = String(i);
    opt.textContent = f.index > 0 ? `${f.question} (${f.index + 1})` : f.question;
    els.rewritePicker.append(opt);
  });
  els.rewritePicker.value = '';
  rewriteTarget = null;
  els.rewriteResult.hidden = true;
  els.rewriteDraft.textContent = '';
}

/** Change handler for the rewrite picker — freezes the picked field's
 *  scan-time correlation (`question`/`index`/`expectedCount`, the SAME
 *  shape `answerFill`/`answerReplace` use) plus its CURRENT text as
 *  {@link rewriteTarget}, so Accept/Restore always act on exactly the field
 *  the user picked, never a moving target. `expectedCount` is the total
 *  number of scanned fields sharing this exact question text — the same
 *  fail-safe correlation `locateFilledField` re-checks on Accept/Restore.
 *
 *  `raw` is checked for emptiness BEFORE the `Number()` coercion —
 *  `Number('')` is `0`, not `NaN`, so a naive `Number.isInteger(Number(raw))`
 *  guard would treat the picker's OWN placeholder (value `''`, selected when
 *  the user picks it back, or on a fresh render) as if index 0 had been
 *  picked, silently re-freezing whatever field happens to be first in
 *  `lastScannedFilled` instead of correctly clearing {@link rewriteTarget}. */
function onRewritePickerChange(): void {
  const raw = els.rewritePicker.value;
  const picked = raw ? lastScannedFilled[Number(raw)] : undefined;
  els.rewriteResult.hidden = true;
  els.rewriteDraft.textContent = '';
  els.rewriteInstruction.value = '';
  if (!picked) {
    rewriteTarget = null;
    return;
  }
  const expectedCount = lastScannedFilled.filter((f) => f.question === picked.question).length;
  rewriteTarget = {
    question: picked.question,
    index: picked.index,
    expectedCount,
    originalAnswer: picked.answer,
    expectedValue: picked.answer,
  };
}

/**
 * Run a rewrite — preset button click (`preset` set, runs immediately,
 * mirrors the in-app `RewritePopover`'s `onPreset`) or the free-text submit
 * button (`preset` omitted, uses the typed instruction). Streams into its
 * own draft box (never `assistDraft`), reusing the SAME `answer.assist`
 * request/streaming buffer as draft mode — `mode: 'rewrite'` plus the picked
 * field's frozen `originalAnswer` as `existingAnswer`. Errors ARE shown (a
 * deliberate click) — mirrors `doAssist`.
 */
async function doRewrite(preset?: ExtensionRewritePreset): Promise<void> {
  if (!rewriteTarget) {
    setMsg(els.importMsg, 'Pick a filled answer first.', 'err');
    return;
  }
  const instruction = els.rewriteInstruction.value.trim();
  if (!preset && !instruction) {
    setMsg(els.importMsg, 'Pick a preset or type an instruction.', 'err');
    return;
  }
  activeAssistKind = 'rewrite';
  els.btnRewrite.disabled = true;
  els.rewriteResult.hidden = true;
  els.rewriteDraft.textContent = '';
  setMsg(els.importMsg, 'Rewriting…', 'muted');
  try {
    const res = await send({
      kind: 'answerAssist',
      question: rewriteTarget.question,
      searchWeb: false,
      mode: 'rewrite',
      existingAnswer: rewriteTarget.originalAnswer,
      ...(preset ? { preset } : {}),
      ...(instruction ? { instruction } : {}),
    });
    const view = resolveAnswerAssistResponse(res);
    setMsg(els.importMsg, view.text, view.tone);
    if (view.draft !== null) {
      // textContent only — this is AI-generated text, never rendered as HTML.
      els.rewriteDraft.textContent = view.draft;
      els.rewriteResult.hidden = false;
    }
  } catch {
    // A transport/messaging rejection must not strand the status on "Rewriting…".
    setMsg(els.importMsg, 'Could not rewrite this answer. Please retry.', 'err');
  } finally {
    els.btnRewrite.disabled = false;
  }
}

/** Click handler for the rewrite draft's Copy button — mirrors
 *  `doCopyAssistDraft` exactly. */
async function doCopyRewriteDraft(): Promise<void> {
  const original = els.btnCopyRewrite.textContent;
  const ok = await copyText(els.rewriteDraft.textContent ?? '');
  els.btnCopyRewrite.textContent = ok ? '✓ Copied' : 'Copy failed';
  setTimeout(() => {
    els.btnCopyRewrite.textContent = original;
  }, 1200);
}

/**
 * Send an `answerReplace` for {@link rewriteTarget} with `text`, showing the
 * outcome in the shared message area — shared by Accept (the rewritten
 * draft) and Restore original (the frozen `originalAnswer`); the ONLY
 * difference between the two is which `text` is passed. Sends the tracked
 * `expectedValue` too, so `replaceFilledField` can refuse (a distinct
 * error, surfaced verbatim below) rather than clobber a manual edit the
 * user made to the field since the pick.
 * Fails safe on any page mutation since the pick (never a different field)
 * — see `replaceFilledField`. Never submits the form.
 *
 * On a SUCCESSFUL write, updates `rewriteTarget.expectedValue` to `text` —
 * the field now holds `text`, not whatever it held before — so the NEXT
 * Accept/Restore compares against the CURRENT baseline instead of a stale
 * one (without this, a successful Accept would make an immediately
 * following Restore wrongly refuse, since the field no longer holds the
 * value this same popup last believed was there).
 */
async function sendRewriteReplace(
  text: string,
  btn: HTMLButtonElement,
  successMsg: string,
  failureFallback: string
): Promise<void> {
  if (!rewriteTarget || !text) return;
  // capture before await: a mid-flight re-pick must not corrupt a different target's expectedValue
  const target = rewriteTarget;
  btn.disabled = true;
  try {
    const res = await send({
      kind: 'answerReplace',
      question: target.question,
      index: target.index,
      count: target.expectedCount,
      text,
      expectedValue: target.expectedValue,
    });
    if (res.ok && res.kind === 'answerReplace' && res.result.filled) {
      target.expectedValue = text;
      setMsg(els.importMsg, successMsg, 'ok');
    } else {
      const msg =
        res.ok && res.kind === 'answerReplace'
          ? (res.result.error ?? failureFallback)
          : !res.ok
            ? res.error
            : failureFallback;
      setMsg(els.importMsg, msg, 'err');
    }
  } catch {
    setMsg(els.importMsg, `${failureFallback} Please retry.`, 'err');
  } finally {
    btn.disabled = false;
  }
}

/** Click handler for "Accept" — writes the rewritten draft onto the picked
 *  field. */
async function doAcceptRewrite(): Promise<void> {
  await sendRewriteReplace(
    els.rewriteDraft.textContent ?? '',
    els.btnAcceptRewrite,
    'Replaced the field on the page.',
    'Could not replace this field.'
  );
}

/** Click handler for "Restore original" — re-injects the FROZEN pre-rewrite
 *  text (never the current draft box), the SAME replace path Accept uses. */
async function doRestoreRewrite(): Promise<void> {
  if (!rewriteTarget) return;
  await sendRewriteReplace(
    rewriteTarget.originalAnswer,
    els.btnRestoreRewrite,
    'Restored the original answer.',
    'Could not restore this field.'
  );
}

/** Build the "Check fit" score card — score / source+résumé line / gap chips.
 *  `textContent` only — no `innerHTML` with page/desktop-derived text. */
function buildMatchResultCard(view: MatchLiveView): HTMLElement {
  const card = document.createElement('div');

  const score = document.createElement('p');
  score.className = 'match-result__score';
  score.textContent = `${view.score}% fit`;
  card.append(score);

  const meta = document.createElement('p');
  meta.className = 'match-result__meta';
  const bits: string[] = [];
  if (view.scoreLabel) bits.push(view.scoreLabel);
  if (view.resumeName) bits.push(`against “${view.resumeName}”`);
  meta.textContent = bits.join(' — ');
  card.append(meta);

  if (view.gaps.length > 0) {
    const gapsWrap = document.createElement('div');
    gapsWrap.className = 'match-result__gaps';
    for (const gap of view.gaps) {
      const chip = document.createElement('span');
      chip.className = 'match-result__gap';
      chip.textContent = gap;
      gapsWrap.append(chip);
    }
    card.append(gapsWrap);
  }

  return card;
}

/** Render the "Check fit" score card — clears any prior card first (no stale
 *  DOM from a previous click). Hidden when there is no score to show (an
 *  error response). */
function renderMatchResult(view: MatchLiveView): void {
  els.matchResult.textContent = '';
  if (view.score === null) {
    els.matchResult.hidden = true;
    return;
  }
  els.matchResult.append(buildMatchResultCard(view));
  els.matchResult.hidden = false;
}

/**
 * Click handler for "Check fit". Scores the default/most-recent résumé
 * against this page's captured posting (keyword coverage only — a local
 * computation; only the score + a few missing keywords ever leave the
 * device). Errors ARE shown (a deliberate click, not a passive check).
 */
async function doCheckFit(): Promise<void> {
  els.btnCheckFit.disabled = true;
  els.matchResult.hidden = true;
  els.matchResult.textContent = '';
  setMsg(els.importMsg, 'Checking fit…', 'muted');
  try {
    const res = await send({ kind: 'matchLive' });
    const view = resolveMatchLiveResponse(res);
    setMsg(els.importMsg, view.text, view.tone);
    renderMatchResult(view);
  } catch {
    // A transport/messaging rejection must not strand the status on "Checking…".
    setMsg(els.importMsg, 'Could not check fit for this page. Please retry.', 'err');
  } finally {
    els.btnCheckFit.disabled = false;
  }
}

async function doImport(): Promise<void> {
  els.btnImport.disabled = true;
  setMsg(els.importMsg, 'Importing…', 'muted');
  try {
    const requestedApplied = els.chkApplied.checked;
    const res = await send({ kind: 'import', applied: requestedApplied });
    const { text, tone } = resolveImportResponse(res, requestedApplied);
    setMsg(els.importMsg, text, tone);
  } catch {
    // A transport/messaging rejection must not strand the status on "Importing…".
    setMsg(els.importMsg, 'Import failed. Please retry.', 'err');
  } finally {
    els.btnImport.disabled = false;
  }
}

async function doFill(): Promise<void> {
  els.btnFill.disabled = true;
  setMsg(els.importMsg, 'Filling…', 'muted');
  try {
    const res = await send({ kind: 'fill' });
    const { text, tone } = resolveFillResponse(res);
    setMsg(els.importMsg, text, tone);
  } catch {
    // A transport/messaging rejection must not strand the status on "Filling…".
    setMsg(els.importMsg, 'Autofill failed. Please retry.', 'err');
  } finally {
    els.btnFill.disabled = false;
  }
}

async function savePairing(): Promise<void> {
  const value = els.tokenInput.value.trim();
  if (!looksLikeToken(value)) {
    setMsg(els.pairMsg, 'That does not look like a 64-character token.', 'err');
    return;
  }
  els.btnSaveToken.disabled = true;
  setMsg(els.pairMsg, 'Pairing…', 'muted');
  try {
    const res = await send({ kind: 'setToken', token: value });
    if (!res.ok) {
      setMsg(els.pairMsg, res.error, 'err');
      els.btnSaveToken.textContent = PAIR_LABEL;
      els.btnSaveToken.disabled = false;
      return;
    }
    // Confirm on the button itself, then flip to the import view after a beat so
    // the "Authorized" state is actually seen (refreshStatus hides the pair view).
    els.btnSaveToken.textContent = '✓ Authorized';
    setMsg(els.pairMsg, 'Paired.', 'ok');
    await delay(AUTHORIZED_CONFIRM_MS);
    await refreshUntilSettled();
    if (!els.views.import.hidden) {
      // Connected view is now shown; move focus off the (hidden) token input.
      els.btnImport.focus();
    } else {
      // Didn't reach the connected view (e.g. app went away) — restore the
      // actionable label so the pair button works again.
      els.btnSaveToken.textContent = PAIR_LABEL;
      els.btnSaveToken.disabled = false;
    }
  } catch {
    // A transport/refresh rejection must never strand the button disabled and
    // labelled "Authorized" — always restore the actionable state.
    setMsg(els.pairMsg, 'Pairing failed. Please retry.', 'err');
    els.btnSaveToken.textContent = PAIR_LABEL;
    els.btnSaveToken.disabled = false;
  }
}

async function unpair(): Promise<void> {
  await send({ kind: 'clearToken' });
  setMsg(els.importMsg, '', 'muted');
  // Restore the pair button to its actionable state for when the view returns.
  els.btnSaveToken.textContent = PAIR_LABEL;
  els.btnSaveToken.disabled = false;
  setMsg(els.pairMsg, '', 'muted');
  await refreshStatus();
  // Pairing view is now shown; move focus off the (hidden) import controls.
  if (!els.views.pair.hidden) els.tokenInput.focus();
}

/** Toggle the help popover open/closed and keep `aria-expanded` in sync. */
function toggleHelp(): void {
  const open = els.helpPopover.hidden;
  els.helpPopover.hidden = !open;
  els.btnHelp.setAttribute('aria-expanded', String(open));
}

async function retry(): Promise<void> {
  await send({ kind: 'reconnect' });
  await refreshStatus();
}

/** Open the desktop app at the extension-pairing settings via the custom URL
 *  scheme. `tabs.create` needs no permission; failures are swallowed so the
 *  popup never shows a disruptive error. */
async function openAppPairing(): Promise<void> {
  try {
    await browser.tabs.create({ url: PAIRING_DEEP_LINK });
  } catch {
    // No-op: the deep link is best-effort; the user can still pair manually.
  }
}

/** Open the public download page in a new tab so a user without the desktop
 *  app can install it. Best-effort; failures are swallowed. */
async function getApp(): Promise<void> {
  try {
    await browser.tabs.create({ url: GET_APP_URL });
  } catch {
    // No-op: best-effort.
  }
}

function wire(): void {
  els.btnImport.addEventListener('click', () => void doImport());
  els.btnFill.addEventListener('click', () => void doFill());
  els.btnMarkApplied.addEventListener('click', () => void doMarkApplied());
  els.btnSaveAnswers.addEventListener('click', () => void doSaveAnswers());
  els.btnSuggestAnswers.addEventListener('click', () => void doSuggestAnswers());
  els.btnCheckFit.addEventListener('click', () => void doCheckFit());
  els.assistPicker.addEventListener('change', () => {
    if (els.assistPicker.value) els.assistQuestion.value = els.assistPicker.value;
  });
  els.btnAssist.addEventListener('click', () => void doAssist());
  els.btnCopyAssist.addEventListener('click', () => void doCopyAssistDraft());
  els.rewritePicker.addEventListener('change', onRewritePickerChange);
  els.rewritePreset.addEventListener('change', () => {
    const preset = els.rewritePreset.value;
    if (!preset) return;
    void doRewrite(preset as ExtensionRewritePreset);
    els.rewritePreset.value = '';
  });
  els.btnRewrite.addEventListener('click', () => void doRewrite());
  els.btnCopyRewrite.addEventListener('click', () => void doCopyRewriteDraft());
  els.btnAcceptRewrite.addEventListener('click', () => void doAcceptRewrite());
  els.btnRestoreRewrite.addEventListener('click', () => void doRestoreRewrite());
  els.btnSaveToken.addEventListener('click', () => void savePairing());
  els.tokenInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') void savePairing();
  });
  els.btnUnpair.addEventListener('click', () => void unpair());
  els.btnRetry.addEventListener('click', () => void retry());
  els.btnOpenSettings.addEventListener('click', () => void openAppPairing());
  els.btnGetApp.addEventListener('click', () => void getApp());
  // The outdated-desktop view sends the user to the same download page (which
  // serves the latest build) to update their app.
  els.btnUpdateApp.addEventListener('click', () => void getApp());
  els.btnHelp.addEventListener('click', toggleHelp);
  // Persist the Answer-tools expand/collapse preference across popup opens —
  // a UI boolean only, not PII/job data. Fires on BOTH a user click on the
  // <summary> and a programmatic `.open` set (e.g. the stream-reattach
  // auto-expand), per the `toggle` event's spec — that is fine here, the
  // stored preference is just "what state it was last left in".
  els.answerTools.addEventListener('toggle', () => {
    void setAnswerToolsExpanded(els.answerTools.open);
  });

  // Live status pushes from the background while the popup is open.
  browser.runtime.onMessage.addListener((message: unknown) => {
    const res = message as PopupResponse;
    if (res && res.ok && res.kind === 'status') render(res.status);
    // Live streaming preview: the background pushes one of these per
    // `assist.chunk` (and once more on settle) while a draft is in flight —
    // update only the dedicated draft box for ongoing chunks, never the
    // shared `importMsg` status line `doAssist` itself owns for this same
    // request. The one exception is the terminal interrupted case: a popup
    // reopened mid-stream has no `doAssist` await of its own to fall back on
    // (see `reattachAssistProgress`), so THIS listener is the only place it
    // ever learns the stream later failed — without this, it would keep
    // showing the last partial draft with no indication it's incomplete.
    // Mirrors `reattachAssistProgress`'s own interruption rendering.
    if (res && res.ok && res.kind === 'answerAssistProgress') {
      // Reflect in-flight state on THIS popup instance too — a popup reattached
      // to a still-running stream (see `reattachAssistProgress`) keeps learning
      // about it here, and must stay disabled until the stream is terminal
      // (never a silent re-trigger that would race/corrupt the shared buffer —
      // see `assistGeneration` in background.ts). Always re-enables once done,
      // so a terminal push never leaves the button stuck disabled. Routed by
      // `activeAssistKind` (draft vs. rewrite, PR 11) to the matching box/button
      // — both modes share the SAME background buffer.
      const btn = activeAssistKind === 'rewrite' ? els.btnRewrite : els.btnAssist;
      btn.disabled = !res.done;
      const view = resolveAssistProgressView(res);
      if (view.draft !== null) {
        if (activeAssistKind === 'rewrite') {
          els.rewriteDraft.textContent = view.draft;
          els.rewriteResult.hidden = false;
        } else {
          els.assistDraft.textContent = view.draft;
          els.assistResult.hidden = false;
        }
      }
      if (view.tone === 'err') setMsg(els.importMsg, view.text, 'err');
    }
  });
}

/**
 * On popup open, reattach to any in-flight/just-finished streaming
 * `answer.assist` the background is holding — so closing the popup
 * mid-stream and reopening shows what already arrived instead of a blank
 * view. No-op when no stream has run this session. Runs BEFORE any user
 * click, so setting the shared `importMsg` line on the interrupted case is
 * safe (nothing else has written to it yet in this fresh popup instance).
 *
 * Also disables `btnAssist` when the reattached stream is still in flight —
 * a freshly-opened popup's button defaults to enabled, and clicking it while
 * the prior run is still streaming would start a second overlapping
 * `runAnswerAssist` (see `assistGeneration` in background.ts). Re-enabled
 * once the reattached stream is terminal, and by the live-push listener
 * above if it finishes while this popup stays open.
 */
async function reattachAssistProgress(): Promise<void> {
  try {
    const res = await send({ kind: 'answerAssistProgress' });
    if (!res.ok || res.kind !== 'answerAssistProgress') return;
    els.btnAssist.disabled = !res.done;
    const view = resolveAssistProgressView(res);
    if (view.draft === null) return;
    // A buffered/still-streaming draft exists from before this popup opened —
    // auto-expand the Answer-tools disclosure so it's visible (never leave the
    // user hunting for a result they already started).
    els.answerTools.open = true;
    els.assistDraft.textContent = view.draft;
    els.assistResult.hidden = false;
    if (view.tone === 'err') setMsg(els.importMsg, view.text, 'err');
  } catch {
    // Best-effort — a transport hiccup on open just means no reattach.
  }
}

/**
 * Apply the persisted Answer-tools expand/collapse preference, THEN check for
 * a reattachable stream — in that order, so an active/buffered draft (which
 * always wins) is never immediately re-collapsed by a stale "collapsed"
 * preference applied after it.
 *
 * Exported (unlike the other `do*`/render helpers) because nothing wires a
 * user click to re-run this bootstrap — it only ever runs once, automatically,
 * at popup load — so it has no other seam for tests to drive it directly.
 */
export async function bootstrapAnswerTools(): Promise<void> {
  try {
    els.answerTools.open = await getAnswerToolsExpanded();
  } catch {
    // Best-effort — a storage read hiccup just keeps the collapsed default.
  }
  await reattachAssistProgress();
}

wire();
void refreshStatusWithTimeout();
void bootstrapAnswerTools();
