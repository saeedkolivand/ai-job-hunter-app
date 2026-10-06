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

import { copyText } from '../answer-tools/answer-tools';
import type { AnswerState } from '../lib/answer-state';
import { getStampResultsPages } from '../lib/appearance';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { buildJobToolsDom, buildMatchResultCard, renderProfileFallbackRows } from './dom';
import {
  buildProfileFallbackFields,
  IMPORT_LABEL_DEFAULT,
  isPageTrusted,
  JOB_TOOLS_GATED_LINE,
  type MatchLiveView,
  resolveAnswersSaveResponse,
  resolveFieldsProbeResponse,
  resolveFillResponse,
  resolveImportResponse,
  resolveMatchLiveResponse,
  resolveStampResultsResponse,
} from './responses';

export * from './responses';

// ── the view ──────────────────────────────────────────────────────────────

export interface JobToolsDeps {
  send: (req: PopupRequest) => Promise<PopupResponse>;
  /** Forwards the fields-probe's `showAnswerTools` half to a caller that owns
   *  its own Answer-tools disclosure. Only the popup does (its `<details>`
   *  element); the panel's Answer-tools section has no such gating today and
   *  simply omits this — adding it there is out of scope for this module. */
  onAnswerToolsVisibility?: (visible: boolean) => void;
  /**
   * Asked BEFORE the Fill request goes out (PR0 §4, first-time Fill
   * confirmation). Resolves `true` to proceed, `false` to cancel — a caller
   * that omits this deps entry gets the old always-proceed behavior (no
   * confirmation), which is what a test double with no site-memory wiring
   * gets for free.
   */
  confirmFill?: () => Promise<boolean>;
  /**
   * Hide "Save my answers from this page" (PR0 §2's three-action rule for the
   * popup launcher — Import / Check fit / Fill only; saving answers moved
   * fully into the panel's Answers tab). Static: set once at mount. Omitted
   * (shown) by the side panel, which keeps all four controls.
   */
  hideSaveAnswers?: boolean;
  /**
   * Copy-field fallback's Copy action. Defaults to the shared `copyText`
   * (`navigator.clipboard.writeText`) — a caller only needs to override this
   * for a test double; both real mounts (`popup.ts`, `sidepanel.ts`) get the
   * default for free, same as the old always-proceed default for
   * `confirmFill`.
   */
  copy?: (text: string) => Promise<boolean>;
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
  const {
    gatedMsg,
    activeWrap,
    formGroup,
    btnImport,
    btnCheckFit,
    btnStampResults,
    btnFill,
    btnSaveAnswers,
    chkApplied,
    matchResult,
    msgEl,
    profileFallback,
  } = buildJobToolsDom(deps.hideSaveAnswers);
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
  /** The tab id this instance last rendered for, or `null` before the first
   *  `render` call — see `render`'s own doc for why page-specific state must
   *  reset on a change of THIS, not only on a change of `trusted`: two
   *  different tabs can both be trusted, and `isPageTrusted` alone cannot
   *  tell them apart. */
  let lastTabId: number | null = null;
  /** A DEDICATED generation for the copy-field fallback's own `profileGet`
   *  fetch (PR review round 2) — sharing `fieldsProbeGeneration` would
   *  invalidate the unrelated fields probe every time the fallback opens.
   *  Bumped by {@link hideProfileFallback}, so every call site that hides the
   *  fallback (`render`'s tab/trust change, `reset()`, a `filled` Fill
   *  result) also discards any `profileGet` reply still in flight for the
   *  PREVIOUS tab/page. */
  let profileFallbackGeneration = 0;

  function setMsg(text: string, tone: 'ok' | 'err' | 'muted'): void {
    msgEl.textContent = text;
    msgEl.className = tone === 'muted' ? 'msg' : `msg msg--${tone}`;
  }

  function redraw(): void {
    gatedMsg.hidden = trusted;
    activeWrap.hidden = !trusted;
    formGroup.hidden = !formGroupVisible;
    // Exactly one solid-red primary CTA per render (popup.css's own
    // `.btn--primary` doc: "the ONE raised primary CTA per context"). Fill
    // outranks Import once the Form group is showing — Import demotes to
    // the quiet tier rather than doubling up on two primaries at once.
    btnImport.className = formGroupVisible ? 'btn btn--quiet' : 'btn btn--primary';
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

  // ── copy-field fallback (decision 8) ─────────────────────────────────────

  /** Copy action for the fallback's per-field buttons — defaults to the
   *  shared `copyText`, same discipline as {@link JobToolsDeps.copy}'s doc. */
  const copyField = deps.copy ?? copyText;

  function hideProfileFallback(): void {
    // Invalidates any `profileGet` reply still in flight (see
    // `profileFallbackGeneration`'s own doc) — every caller of this function
    // (render's tab/trust change, reset(), a `filled` Fill result) counts as
    // leaving the fallback this fetch was for.
    profileFallbackGeneration += 1;
    profileFallback.hidden = true;
    profileFallback.replaceChildren();
  }

  /**
   * Fetch the profile fresh (`profileGet` — the same source + Autofill
   * opt-in gate `fill` itself uses) and render it as Copy-able fields.
   * Fail-closed: any refusal, error, or empty profile renders nothing (never
   * stored, never retried automatically).
   */
  async function showProfileFallback(): Promise<void> {
    const myGeneration = profileFallbackGeneration;
    try {
      const res = await deps.send({ kind: 'profileGet' });
      // A tab switch / trust change / reset landed while this was in
      // flight — that already hid the fallback for whatever page this now
      // is; a stale reply must never resurrect it (PR review round 2).
      if (myGeneration !== profileFallbackGeneration) return;
      if (res.ok && res.kind === 'profileGet') {
        renderProfileFallbackRows(
          profileFallback,
          buildProfileFallbackFields(res.result),
          copyField
        );
      } else {
        hideProfileFallback();
      }
    } catch {
      if (myGeneration !== profileFallbackGeneration) return;
      hideProfileFallback();
    }
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
    // Lock the button BEFORE awaiting the (possibly slow, user-facing)
    // confirmation — a repeated click while it's pending must not start a
    // second concurrent confirmation or double-send `fill` once the first
    // resolves.
    if (btnFill.disabled) return;
    btnFill.disabled = true;
    try {
      if (deps.confirmFill) {
        const proceed = await deps.confirmFill();
        if (!proceed) return;
      }
      setMsg('Filling…', 'muted');
      const res = await deps.send({ kind: 'fill' });
      const { text, tone } = resolveFillResponse(res);
      setMsg(text, tone);
      if (res.ok && res.kind === 'fill' && res.summary.filledNothing) {
        await showProfileFallback();
      } else {
        hideProfileFallback();
      }
    } catch {
      // A transport/messaging rejection must not strand the status on "Filling…".
      setMsg('Autofill failed. Please retry.', 'err');
    } finally {
      btnFill.disabled = false;
    }
  }

  async function doStampResults(): Promise<void> {
    btnStampResults.disabled = true;
    setMsg('Stamping…', 'muted');
    try {
      const res = await deps.send({ kind: 'stampResults' });
      const { text, tone } = resolveStampResultsResponse(res);
      setMsg(text, tone);
    } catch {
      setMsg('Could not stamp this page. Please retry.', 'err');
    } finally {
      btnStampResults.disabled = false;
    }
  }

  /** Re-read the results-stamp preference — called on mount and every
   *  `checkPage()` (tab show/activation), never cached: R7/PR3 "the
   *  panel/popup must honour them live". */
  async function refreshStampResultsVisibility(): Promise<void> {
    btnStampResults.hidden = !(await getStampResultsPages());
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
  btnStampResults.addEventListener('click', () => void doStampResults());
  btnFill.addEventListener('click', () => void doFill());
  btnSaveAnswers.addEventListener('click', () => void doSaveAnswers());

  // Popup: a fresh mount per open already reads the current preference once;
  // the panel additionally re-reads it on every `checkPage()` below.
  void refreshStampResultsVisibility();

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
    void refreshStampResultsVisibility();
  }

  // ── the trust gate ──────────────────────────────────────────────────────

  /**
   * The dedup guard is TWO conditions, not one: `trusted` alone cannot tell
   * two different (both-trusted) tabs apart, so switching the panel from an
   * already-gestured tab A to an already-gestured tab B must still reset
   * page-specific state — a stale Check-fit score or Form-group visibility
   * from A must never linger on top of B's page, even though `isPageTrusted`
   * returns `true` for both. `lastTabId` is what catches that case; `trusted`
   * alone would dedup it away.
   */
  function render(state: AnswerState | null): void {
    const next = isPageTrusted(state);
    const nextTabId = state?.tabId ?? null;
    if (next === trusted && nextTabId === lastTabId) return;
    trusted = next;
    lastTabId = nextTabId;

    // Page-specific state belongs to whichever tab this instance last
    // rendered — reset it on ANY change captured above, not only when trust
    // flips to false. `formGroupVisible` resets to the fail-open default
    // (matching `reset()`) rather than lingering on the PREVIOUS tab's
    // answer until this tab's own probe (below) resolves.
    fieldsProbeGeneration += 1;
    matchResult.hidden = true;
    matchResult.textContent = '';
    hideProfileFallback();
    formGroupVisible = true;
    deps.onAnswerToolsVisibility?.(true);
    if (trusted) {
      // A live grant just landed while this instance stayed mounted (the
      // panel never remounts on its own), OR this is a DIFFERENT
      // already-trusted tab — either way, refresh the fields-gated group for
      // whatever page this now is.
      checkPage();
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
    hideProfileFallback();
    setMsg('', 'muted');
    redraw();
  }

  return { render, checkPage, setImportLabel, reset };
}
