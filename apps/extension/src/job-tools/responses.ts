/**
 * Pure decisions for the job-tools controls — the trust gate and every
 * response-to-message shaper (import, fill, check fit, stamp, save answers,
 * fields probe). No DOM access, no side effects; re-exported unchanged from
 * `job-tools.ts`, whose doc explains the trust gate's contract.
 */

import type { ExtensionProfileResult } from '@ajh/shared';

import type { AnswerState } from '../lib/answer-state';
import type { PopupResponse } from '../lib/messages';

/** The status line for a reply whose `kind` is not the one the request expects. */
const UNEXPECTED = 'Unexpected response — please retry.';

/** What every `resolve*Response` hands the status line. */
interface StatusLine {
  text: string;
  tone: 'ok' | 'err';
}

// ── the trust gate ────────────────────────────────────────────────────────

/**
 * Whether the panel's currently-followed tab has a record saying a
 * qualifying gesture landed since its last navigation. "No record" is
 * equivalent to `pageChanged: true` (untrusted) — under-claiming is the safe
 * direction, same rationale as `AnswerState.pageChanged`'s own doc.
 *
 * Pure — no DOM, no side effects.
 */
export function isPageTrusted(state: AnswerState | null): boolean {
  return state !== null && !state.pageChanged;
}

/** The line that replaces all four controls when {@link isPageTrusted} is
 *  false — same convention as `answer-tools.ts`'s `PAGE_CHANGED_LINE`. */
export const JOB_TOOLS_GATED_LINE =
  'Click the toolbar icon to grant access to this page, then use these tools.';

// ── Import ────────────────────────────────────────────────────────────────

/** Where an imported job lands in the desktop app — shown on success so the
 *  user knows where to look (the extension can't focus the native window). */
const IMPORT_LANDING_HINT = 'Open AI Job Hunter → Applications to view it.';

/** Shown when the job was saved but the description couldn't be read. */
const IMPORT_PARTIAL_HINT = 'Open AI Job Hunter → Applications to paste it.';

/** Percent-fit suffix appended to the import success/status-unchanged lines
 *  when the desktop populated `matchScore` (a best-effort keyword-only
 *  score, omitted on failure) — mirrors the "Check fit" card's percent
 *  treatment without the résumé name the import reply doesn't carry. */
function matchScoreSuffix(matchScore: number | undefined): string {
  return typeof matchScore === 'number' ? ` — ${Math.round(matchScore)}% fit.` : '';
}

/** Default label for the Import button. The adaptive "Re-import / update"
 *  relabel lives in popup.ts (its own, unmoved `appliedCheck` auto-check) —
 *  exported here purely so that logic can compare/apply it without a second
 *  copy of the literal. */
export const IMPORT_LABEL_DEFAULT = 'Import this job';
export const IMPORT_LABEL_FOUND = 'Re-import / update';

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
 * like the status had changed when only the status was left untouched — surface
 * that explicitly instead.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveImportResponse(res: PopupResponse, requestedApplied: boolean): StatusLine {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'import') return { text: UNEXPECTED, tone: 'err' };
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
    const label = result.status.charAt(0).toUpperCase() + result.status.slice(1);
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

// ── Fill ──────────────────────────────────────────────────────────────────

/**
 * Given a `fill` response, return the popup message + tone. The detailed
 * summary lives in the in-page overlay; this shows a short confirmation (or
 * the desktop's refusal when autofill is opted out). Handles the "nothing
 * matched" case explicitly so a no-op never reads as a failure.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveFillResponse(res: PopupResponse): StatusLine {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'fill') return { text: UNEXPECTED, tone: 'err' };
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

/** One profile field the copy-field fallback renders. */
export interface ProfileFallbackField {
  label: string;
  value: string;
}

/**
 * Copy-field fallback (decision 8): when Fill finds nothing to match, project
 * a `profileGet` result into the field list the Job tab shows instead — each
 * with a Copy button, never stored. A refusal/failure (`result.error` set)
 * or an empty profile both project to `[]`, which the caller renders as
 * nothing (the fallback's own fail-closed discipline, same as `runFill`'s).
 *
 * Pure: no DOM access, no side effects.
 */
export function buildProfileFallbackFields(result: ExtensionProfileResult): ProfileFallbackField[] {
  if (result.error) return [];
  const fields: ProfileFallbackField[] = [];
  const push = (label: string, value: string | undefined): void => {
    if (value?.trim()) fields.push({ label, value });
  };
  push('Name', result.fullName);
  push('Email', result.email);
  push('Phone', result.phone);
  push('Location', result.location);
  push('LinkedIn', result.linkedin);
  push('GitHub', result.github);
  push('Website', result.website);
  for (const link of result.extraLinks ?? []) push(link.label, link.url);
  return fields;
}

// ── Check fit ─────────────────────────────────────────────────────────────

/** Human-readable label for `scoreSource` — `'combined'` is wire-reserved and
 *  never sent by the current desktop (keyword-only always), but the label
 *  exists so a future desktop's value renders sensibly without a change here. */
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
  /** PR3 — two verbatim salary facts, never a verdict (design decision 5).
   *  `undefined` when the desktop found no range and no stored expectation. */
  salary?: { posting: string; expectation?: string };
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
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveMatchLiveResponse(res: PopupResponse): MatchLiveView {
  if (!res.ok) return NO_MATCH_VIEW(res.error, 'err');
  if (res.kind !== 'matchLive') {
    return NO_MATCH_VIEW(UNEXPECTED, 'err');
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
    salary: result.salary,
  };
}

/** Qualitative band next to the score (R6 of the redesign record) — same
 *  bands the popup/panel mockups use: strong ≥ 80, partial 50–79, low < 50. */
export function scoreBand(score: number): 'strong match' | 'partial match' | 'low match' {
  if (score >= 80) return 'strong match';
  if (score >= 50) return 'partial match';
  return 'low match';
}

// ── Stamp this results page (PR3 §B.4) ──────────────────────────────────────

/**
 * Given a `stampResults` response, return the message text + tone. UNLIKE
 * `resolveAnswersSaveResponse`, a desktop-side refusal is NOT surfaced as
 * `err` — `PopupResponse`'s `stampResults` doc: any refusal short of "not
 * paired"/"no active tab" degrades to `ok:true, stamped:0` with an
 * explanatory `status`, which reads as a neutral/ok status line here too.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveStampResultsResponse(res: PopupResponse): StatusLine {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'stampResults') {
    return { text: UNEXPECTED, tone: 'err' };
  }
  return { text: res.status, tone: 'ok' };
}

// ── Save my answers ───────────────────────────────────────────────────────

/**
 * Given an `answersSave` response, return the message text + tone. On
 * success names the job from the reply's `title`/`company` and reports the
 * saved count; a re-capture with nothing new to add reads as a benign "no
 * new answers", never an error. When the desktop dedupes/caps some answers,
 * `skipped` is folded into the copy too — `saved === 0` gets a distinct
 * "already recorded" message instead of the generic no-new-answers one.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveAnswersSaveResponse(res: PopupResponse): StatusLine {
  if (!res.ok) return { text: res.error, tone: 'err' };
  if (res.kind !== 'answersSave') {
    return { text: UNEXPECTED, tone: 'err' };
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

// ── Form-group visibility (fields probe) ─────────────────────────────────

/** Whether the Form group (Fill + Save answers) should show — see
 *  `resolveFieldsProbeResponse`'s doc for the split with `showAnswerTools`. */
export interface FieldsProbeView {
  showFormGroup: boolean;
  showAnswerTools: boolean;
}

/**
 * Given a `fieldsProbe` response, whether the Form group and the caller's
 * Answer-tools disclosure should each be shown. Fails OPEN (`true` for both)
 * on a transport-level `ok:false` or an unexpected `kind` — mirrors the
 * background's own fail-open fold (`runFieldsProbe`) so a probe bug can never
 * hide either feature; only a CONFIRMED `false` signal hides one.
 *
 * Pure: no DOM access, no side effects.
 */
export function resolveFieldsProbeResponse(res: PopupResponse): FieldsProbeView {
  if (!res.ok || res.kind !== 'fieldsProbe') {
    return { showFormGroup: true, showAnswerTools: true };
  }
  return { showFormGroup: res.hasFormFields, showAnswerTools: res.hasAnswerFields };
}
