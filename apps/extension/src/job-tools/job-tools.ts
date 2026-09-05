/**
 * The four page-scoped job-tools controls — Import this job / Check fit /
 * Fill this form / Save my answers from this page — mounted by BOTH the
 * popup and the side panel (full parity: the panel previously had none of
 * them at all). Moved out of popup.ts essentially unchanged: same wire calls
 * (`send({kind:'import'|'matchLive'|'fill'|'answersSave'})`), same response
 * handling, same button disable-during-request pattern — only the DOM target
 * changed (an injected `host`, not popup.html's specific element ids).
 *
 * "Mark as applied" and the adaptive Import re-label stay OUT of this module
 * and in popup.ts, unmoved: they were never part of the requested parity
 * (only the four verbs above), and showing them in the panel would be a
 * capability the panel never had. The one seam the popup's own (unmoved)
 * `appliedCheck` auto-check still needs into this module is
 * {@link JobToolsView.setImportLabel}, so the Import button this module now
 * owns can still carry the adaptive re-import wording; {@link
 * JobToolsView.reset} mirrors the rest of what that auto-check used to reset
 * directly on `els.*` (the match-fit card, the Form group's visibility).
 *
 * ## The trust gate (new — side panel only in practice)
 *
 * Chrome's `activeTab` permission is granted only by a fresh user gesture
 * (toolbar click, context-menu click, …) — never by clicking a control
 * already rendered inside an open side panel. The popup is always freshly
 * gestured (opening it IS the gesture), so it never needs this; the panel
 * persists across tab switches with no equivalent per-switch gesture. Each of
 * the four controls' underlying background call needs a LIVE grant to work
 * (`captureActiveTabFieldsProbe`/`activeTabUrl` both call into
 * `browser.scripting`/`browser.tabs` under it), so {@link isPageTrusted}
 * gates them: untrusted replaces all four with one line instead of merely
 * disabling them (a disabled button still claims the capability exists).
 *
 * The gate is derived from the SAME `AnswerState.pageChanged` ADR-044 already
 * uses for the Answer-tools write controls — see that module's doc for why
 * the record is scoped per (tab, origin) and re-armed only by a real gesture.
 *
 * This module can only enforce the gate against the trust value it currently
 * holds — it has no way to know a caller just switched to a DIFFERENT tab
 * until that tab's own `AnswerState` is actually delivered to {@link
 * JobToolsView.render}. So the caller carries half of this contract: {@link
 * JobToolsView.checkPage} must never be called as a bare next statement after
 * subscribing to a newly-followed tab's state (that subscription's first
 * delivery is unavoidably asynchronous), only from inside that delivery,
 * after `render` has already run for it — see `sidepanel.ts::follow`'s doc,
 * the one caller this currently matters for.
 */

import type { AnswerState } from '../lib/answer-state';
import type { PopupRequest, PopupResponse } from '../lib/messages';

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

// ── the view ──────────────────────────────────────────────────────────────

export interface JobToolsDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
  /** Forwards the fields-probe's `showAnswerTools` half to a caller that owns
   *  its own Answer-tools disclosure. Only the popup does (its `<details>`
   *  element); the panel's Answer-tools section has no such gating today and
   *  simply omits this — adding it there is out of scope for this module. */
  onAnswerToolsVisibility?: (visible: boolean) => void;
}

export interface JobToolsView {
  /** Feed the latest per-tab `AnswerState` so the trust gate can decide
   *  whether the four controls render as active or as
   *  {@link JOB_TOOLS_GATED_LINE}. Only the panel calls this — the popup
   *  structurally never needs the gate (see this module's doc) and must
   *  never call it, since its own AnswerState subscription can otherwise
   *  read `null` before its first scan lands and wrongly gate a surface that
   *  is always freshly gestured. Cheap to call on every state push — it only
   *  redraws when trust actually changes, and re-runs the fields probe when
   *  it flips from untrusted to trusted (a live grant regained while this
   *  instance stayed mounted, which the panel — unlike the popup — never
   *  remounts on its own to pick up otherwise). */
  render: (state: AnswerState | null) => void;
  /** Run the fields probe on the surface's own trigger (popup: the bridge's
   *  connect-phase transition; panel: mount + tab activation) — a no-op when
   *  the gate currently reads untrusted, so an ungated call is never made for
   *  a tab that cannot safely answer it. This reads whatever `trusted`
   *  CURRENTLY holds — it does not itself wait for a fresh `AnswerState`, so
   *  a caller that just switched to a different tab must call `render` with
   *  that tab's own state FIRST (synchronously in the same callback, not as a
   *  separate statement racing an async read) or this will run — or skip —
   *  based on the PREVIOUS tab's trust instead. */
  checkPage: () => void;
  /** Override the Import button's label — used ONLY by the popup's own
   *  (unmoved) `appliedCheck` auto-check for the adaptive re-import wording;
   *  this module has no opinion on it otherwise. */
  setImportLabel: (label: string) => void;
  /** Reset to the disconnected/no-page defaults: the popup's own connection
   *  status render calls this on leaving `connected`, mirroring what its
   *  `appliedCheck` auto-check used to reset directly on `els.*` for the
   *  pieces this module now owns (the Import label, the match-fit card, the
   *  Form group's visibility). */
  reset: () => void;
}

/**
 * Mount the four job-tools controls into `host`. Both the popup and the side
 * panel call this against the SAME `deps.send`, so the two surfaces are two
 * views of one background, never two implementations.
 */
export function mountJobTools(host: HTMLElement, deps: JobToolsDeps): JobToolsView {
  // ── DOM ─────────────────────────────────────────────────────────────────
  const gatedMsg = document.createElement('p');
  gatedMsg.id = 'job-tools-gated';
  gatedMsg.className = 'msg msg--muted';
  gatedMsg.setAttribute('role', 'status');
  gatedMsg.setAttribute('aria-live', 'polite');
  gatedMsg.textContent = JOB_TOOLS_GATED_LINE;

  const activeWrap = document.createElement('div');
  activeWrap.id = 'job-tools-active';

  const jobGroup = document.createElement('section');
  jobGroup.className = 'group';
  jobGroup.setAttribute('aria-label', 'Job');

  const btnImport = document.createElement('button');
  btnImport.id = 'btn-import';
  btnImport.type = 'button';
  btnImport.className = 'btn btn--primary';
  btnImport.textContent = IMPORT_LABEL_DEFAULT;

  const btnCheckFit = document.createElement('button');
  btnCheckFit.id = 'btn-check-fit';
  btnCheckFit.type = 'button';
  btnCheckFit.className = 'btn btn--quiet';
  btnCheckFit.title = "Score your resume against this page's job posting";
  btnCheckFit.textContent = 'Check fit';

  const matchResult = document.createElement('div');
  matchResult.id = 'match-result';
  matchResult.className = 'match-result';
  matchResult.hidden = true;

  const chkApplied = document.createElement('input');
  chkApplied.id = 'chk-applied';
  chkApplied.type = 'checkbox';
  const chkLabel = document.createElement('label');
  chkLabel.className = 'check';
  const chkSpan = document.createElement('span');
  chkSpan.textContent = 'I already applied to this job';
  chkLabel.append(chkApplied, chkSpan);

  jobGroup.append(btnImport, btnCheckFit, matchResult, chkLabel);

  const formGroup = document.createElement('section');
  formGroup.id = 'group-form';
  formGroup.className = 'group group--divided';
  formGroup.setAttribute('aria-label', 'Form');

  const btnFill = document.createElement('button');
  btnFill.id = 'btn-fill';
  btnFill.type = 'button';
  btnFill.className = 'btn btn--primary';
  btnFill.title =
    "Fill this page's form with your saved contact details (opt-in, review before submitting)";
  btnFill.textContent = 'Fill this form';

  const btnSaveAnswers = document.createElement('button');
  btnSaveAnswers.id = 'btn-save-answers';
  btnSaveAnswers.type = 'button';
  btnSaveAnswers.className = 'btn btn--quiet';
  btnSaveAnswers.title = "Save the answers you typed on this page's application form";
  btnSaveAnswers.textContent = 'Save my answers from this page';

  formGroup.append(btnFill, btnSaveAnswers);

  const msgEl = document.createElement('p');
  msgEl.id = 'job-tools-msg';
  msgEl.className = 'msg';
  msgEl.setAttribute('role', 'status');
  msgEl.setAttribute('aria-live', 'polite');

  activeWrap.append(jobGroup, formGroup, msgEl);
  host.append(gatedMsg, activeWrap);

  // ── state ───────────────────────────────────────────────────────────────
  // Starts trusted and STAYS trusted unless a caller feeds `render` a
  // navigated/absent AnswerState — the popup deliberately never calls
  // `render` at all (see this module's doc: it structurally never needs the
  // gate), so its instance never flips. The panel does call it, once its
  // subscription's first (unavoidably async) delivery lands — see
  // `sidepanel.ts::follow`'s doc for why `checkPage()` must never be called
  // before that delivery, and why THIS default therefore only ever affects
  // what briefly renders before it (active controls, not the gated line, for
  // the very first paint of a freshly-mounted instance), never whether an
  // ungated probe call can reach a tab this module has not yet evaluated.
  let trusted = true;
  let formGroupVisible = true;
  let fieldsProbeGeneration = 0;

  function setMsg(text: string, tone: 'ok' | 'err' | 'muted'): void {
    msgEl.textContent = text;
    msgEl.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
  }

  function redraw(): void {
    gatedMsg.hidden = trusted;
    activeWrap.hidden = !trusted;
    formGroup.hidden = !formGroupVisible;
  }
  redraw();

  function renderMatchResult(view: MatchLiveView): void {
    matchResult.textContent = '';
    if (view.score === null) {
      matchResult.hidden = true;
      return;
    }
    matchResult.append(buildMatchResultCard(view));
    matchResult.hidden = false;
  }

  // ── the four actions (moved essentially unchanged from popup.ts) ──────────

  async function doImport(): Promise<void> {
    btnImport.disabled = true;
    setMsg('Importing…', 'muted');
    try {
      const requestedApplied = chkApplied.checked;
      const res = await deps.send({ kind: 'import', applied: requestedApplied });
      const { text, tone } = resolveImportResponse(res, requestedApplied);
      setMsg(text, tone);
    } catch {
      // A transport/messaging rejection must not strand the status on "Importing…".
      setMsg('Import failed. Please retry.', 'err');
    } finally {
      btnImport.disabled = false;
    }
  }

  async function doCheckFit(): Promise<void> {
    btnCheckFit.disabled = true;
    matchResult.hidden = true;
    matchResult.textContent = '';
    setMsg('Checking fit…', 'muted');
    try {
      const res = await deps.send({ kind: 'matchLive' });
      const view = resolveMatchLiveResponse(res);
      setMsg(view.text, view.tone);
      renderMatchResult(view);
    } catch {
      // A transport/messaging rejection must not strand the status on "Checking…".
      setMsg('Could not check fit for this page. Please retry.', 'err');
    } finally {
      btnCheckFit.disabled = false;
    }
  }

  async function doFill(): Promise<void> {
    btnFill.disabled = true;
    setMsg('Filling…', 'muted');
    try {
      const res = await deps.send({ kind: 'fill' });
      const { text, tone } = resolveFillResponse(res);
      setMsg(text, tone);
    } catch {
      // A transport/messaging rejection must not strand the status on "Filling…".
      setMsg('Autofill failed. Please retry.', 'err');
    } finally {
      btnFill.disabled = false;
    }
  }

  async function doSaveAnswers(): Promise<void> {
    btnSaveAnswers.disabled = true;
    setMsg('Saving your answers…', 'muted');
    try {
      const res = await deps.send({ kind: 'answersSave' });
      const { text, tone } = resolveAnswersSaveResponse(res);
      setMsg(text, tone);
    } catch {
      // A transport/messaging rejection must not strand the status on "Saving…".
      setMsg('Could not save your answers. Please retry.', 'err');
    } finally {
      btnSaveAnswers.disabled = false;
    }
  }

  btnImport.addEventListener('click', () => void doImport());
  btnCheckFit.addEventListener('click', () => void doCheckFit());
  btnFill.addEventListener('click', () => void doFill());
  btnSaveAnswers.addEventListener('click', () => void doSaveAnswers());

  // ── fields probe (gated on trust) ──────────────────────────────────────

  /**
   * Fire-and-forget "does this page have fillable form fields?" probe,
   * gating the Form group (+ the caller's own Answer-tools disclosure, via
   * {@link JobToolsDeps.onAnswerToolsVisibility}) on the result. Mirrors
   * `runFieldsProbe`'s always-`ok:true`, fail-OPEN fold: any transport-level
   * rejection here resolves both signals `true` so a probe bug can never
   * hide either feature.
   */
  async function runFieldsProbeCheck(): Promise<void> {
    fieldsProbeGeneration += 1;
    const myGeneration = fieldsProbeGeneration;
    try {
      const res = await deps.send({ kind: 'fieldsProbe' });
      if (myGeneration !== fieldsProbeGeneration) return;
      const view = resolveFieldsProbeResponse(res);
      formGroupVisible = view.showFormGroup;
      deps.onAnswerToolsVisibility?.(view.showAnswerTools);
      redraw();
    } catch {
      if (myGeneration !== fieldsProbeGeneration) return;
      formGroupVisible = true;
      deps.onAnswerToolsVisibility?.(true);
      redraw();
    }
  }

  function checkPage(): void {
    if (!trusted) return;
    void runFieldsProbeCheck();
  }

  // ── the trust gate ──────────────────────────────────────────────────────

  function render(state: AnswerState | null): void {
    const next = isPageTrusted(state);
    if (next === trusted) return;
    trusted = next;
    if (trusted) {
      // A live grant just landed while this instance stayed mounted (the
      // panel never remounts on its own) — refresh the fields-gated group
      // for whatever page this now is.
      checkPage();
    } else {
      // The page we were showing results for is gone — a stale in-flight
      // probe (or the score card it would render) must not survive it.
      fieldsProbeGeneration += 1;
      matchResult.hidden = true;
      matchResult.textContent = '';
    }
    setMsg('', 'muted');
    redraw();
  }

  function setImportLabel(label: string): void {
    btnImport.textContent = label;
  }

  function reset(): void {
    fieldsProbeGeneration += 1;
    formGroupVisible = true;
    deps.onAnswerToolsVisibility?.(true);
    btnImport.textContent = IMPORT_LABEL_DEFAULT;
    matchResult.hidden = true;
    matchResult.textContent = '';
    setMsg('', 'muted');
    redraw();
  }

  return { render, checkPage, setImportLabel, reset };
}
