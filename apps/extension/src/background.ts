/**
 * Background service worker / event page.
 *
 * Owns the single {@link BridgeClient} to the desktop loopback bridge and
 * answers the popup's `runtime.onMessage` requests. MV3 lifecycle: this context
 * can be evicted whenever idle, so all state is reconstructed lazily on wake
 * (`getClient()`), and we re-probe on `runtime.onStartup`, `onInstalled`, and
 * whenever the popup sends its first message.
 */

import { type Browser, browser } from '@wxt-dev/browser';

import type {
  ExtensionAnswerAssistRequest,
  ExtensionAnswersSaveResult,
  ExtensionDocumentSource,
  ExtensionImportRequest,
  ExtensionMatchLiveRequest,
  ExtensionMatchLiveResult,
  ExtensionRewritePreset,
  ExtensionSettingsKey,
} from '@ajh/shared';
// Runtime import, so it comes from the dedicated entrypoint rather than the
// barrel — see the note at the top of `answer-tools/answer-tools.ts`.
import { EXTENSION_ANSWER_ASSIST_MAX_CHARS } from '@ajh/shared/extension-protocol';

// TYPE-ONLY import from the answer-fill module — same rationale as the
// autofill.ts import below: `answer-fill.js`/`answer-replace.js` are
// classic-script injection targets, so their runtime code must be imported
// ONLY by `answer-fill.ts`/`answer-replace.ts`.
import type { FillAnswerResult } from './lib/answer-fill';
import {
  addFreeRow,
  type AnswerRow,
  type AnswerScan,
  type AnswerState,
  appendVersion,
  buildRows,
  clearAnswerState,
  isUnchangedRewrite,
  readAnswerState,
  rewriteBaseText,
  selectedText,
  updateAnswerState,
  writeAnswerState,
} from './lib/answer-state';
// Same type-only rationale as the autofill.ts import above — capture.js /
// capture-questions.js are ALSO classic-script injection targets (see
// vite.config.mts's `injectedEntries`), so this import must stay type-only
// (erased at build) to keep answers-capture.ts's runtime code out of the
// background's bundle.
import type { CapturedAnswer, FilledField, ScannedQuestion } from './lib/answers-capture';
import { getShowFitBadge, getStampResultsPages } from './lib/appearance';
// TYPE-ONLY import — same rationale as the autofill.ts import below:
// `attach-file.js` is a classic-script injection target (see
// `injected-entries.mjs`), so its runtime code (`runAttachFile`,
// `ATTACH_FILE_GLOBAL`) must be imported ONLY by `attach-file.ts`. The
// global key is duplicated as a local literal below, same discipline as
// `AUTOFILL_GLOBAL`.
import type { AttachFileResult } from './lib/attach-file';
import { setAutoSaveNotice, takeAutoSaveNotice } from './lib/auto-save-notice';
import { handleSubmitDetected, maybeArmSubmitWatch } from './lib/auto-track';
// TYPE-ONLY import from the autofill module. This is deliberate: `fill.js` is
// injected via `executeScript({ files })`, which runs as a CLASSIC script (no ES
// modules) — so `fill.js` must bundle with ZERO `import` statements. If the
// background also imported autofill.ts at RUNTIME, Rollup would hoist it into a
// shared chunk that `fill.js` then `import`s, breaking injection. Keeping this
// type-only (elided at build) means autofill.ts is runtime-imported ONLY by
// fill.ts and gets inlined into a self-contained `fill.js`. The tiny runtime
// bits the background needs (the global key + a result guard) are defined below.
import type { AutofillProfile, AutofillSummary } from './lib/autofill';
import { BridgeClient } from './lib/bridge';
import { stripFenceWrapper } from './lib/fence-strip';
// TYPE-ONLY import — same rationale as the autofill.ts import above:
// `fit-badge.js` is a classic-script injection target (see
// `injected-entries.mjs`), so its runtime code (`runRenderFitBadge`) must be
// imported ONLY by `fit-badge.ts`. The global key is duplicated as a local
// literal below, same discipline as `AUTOFILL_GLOBAL`.
import type { FitBadgeView } from './lib/fit-badge';
import type { ConnectionStatus, PopupRequest, PopupResponse } from './lib/messages';
// TYPE-ONLY import — same rationale as `fit-badge.ts` above: `results-
// stamp.js` is a classic-script injection target, so its runtime code must
// be imported ONLY by `results-stamp.ts`. Both global keys are duplicated
// as local literals below.
import type { CollectedCard, StampInput } from './lib/results-stamp';
import { clearToken, getToken, looksLikeToken, setToken } from './lib/storage';

/**
 * Isolated-world global key under which `fill.js` exposes the filler. MUST match
 * `AUTOFILL_GLOBAL` in `lib/autofill.ts` (pinned by a test there). Duplicated as a
 * local literal — not imported — so autofill.ts stays out of the background's
 * runtime graph (see the type-only import note above).
 */
const AUTOFILL_GLOBAL = '__ajhRunAutofill';

/** Isolated-world global key under which `answer-fill.js` exposes the filler.
 *  MUST match `ANSWER_FILL_GLOBAL` in `lib/answer-fill.ts` (pinned by a test
 *  there). Duplicated as a local literal for the same reason as
 *  `AUTOFILL_GLOBAL` above. */
const ANSWER_FILL_GLOBAL = '__ajhRunAnswerFill';

/** Isolated-world global key under which `answer-replace.js` exposes the
 *  replacer (PR 11's rewrite Accept/Restore). MUST match
 *  `ANSWER_REPLACE_GLOBAL` in `lib/answer-fill.ts` (pinned by a test there).
 *  Duplicated as a local literal for the same reason as `AUTOFILL_GLOBAL`
 *  above. */
const ANSWER_REPLACE_GLOBAL = '__ajhRunAnswerReplace';

/** Isolated-world global key under which `attach-file.js` exposes the
 *  runner (PR2). MUST match `ATTACH_FILE_GLOBAL` in `lib/attach-file.ts`.
 *  Duplicated as a local literal for the same reason as `AUTOFILL_GLOBAL`
 *  above. */
const ATTACH_FILE_GLOBAL = '__ajhRunAttachFile';

/** Isolated-world global key under which `fit-badge.js` exposes the
 *  renderer (PR3). MUST match `FIT_BADGE_GLOBAL` in `lib/fit-badge.ts`.
 *  Duplicated as a local literal for the same reason as `AUTOFILL_GLOBAL`
 *  above. */
const FIT_BADGE_GLOBAL = '__ajhRenderFitBadge';

/** Internal message kind the fit badge's "Open the panel" button posts —
 *  MUST match `OPEN_PANEL_MSG` in `lib/fit-badge.ts`. Duplicated as a local
 *  literal for the same reason as `SUBMIT_DETECTED_MSG` below. */
const OPEN_PANEL_FROM_BADGE_MSG = 'ajhOpenPanelFromBadge';

/** Isolated-world global keys under which `results-stamp.js` exposes the
 *  collector/stamper (PR3). MUST match `RESULTS_COLLECT_GLOBAL`/
 *  `RESULTS_STAMP_GLOBAL` in `lib/results-stamp.ts`. Duplicated as local
 *  literals for the same reason as `AUTOFILL_GLOBAL` above. */
const RESULTS_COLLECT_GLOBAL = '__ajhCollectResultsCards';
const RESULTS_STAMP_GLOBAL = '__ajhStampResultsCards';

/** Isolated-world global key under which `submit-watch.js` exposes its arm
 *  runner (PR4). MUST match `SUBMIT_WATCH_GLOBAL` in `lib/submit-watch.ts`.
 *  Duplicated as a local literal for the same reason as `AUTOFILL_GLOBAL`
 *  above. */
const SUBMIT_WATCH_GLOBAL = '__ajhArmSubmitWatch';

/** Internal message kind the injected `submit-watch.js` posts on a detected
 *  form submit (Task #22). Duplicated as a local literal — MUST match
 *  `SUBMIT_DETECTED_MSG` in `lib/submit-watch.ts` — so that pure DOM module
 *  (and its `field-signal` dependency) never bundles into the background's
 *  runtime graph (same discipline as `AUTOFILL_GLOBAL` above). Exported ONLY
 *  so `background.test.ts` can pin this literal against the imported const —
 *  a future edit to one side can't silently break routing. */
export const SUBMIT_DETECTED_MSG = 'submitDetected';

/** Popup requests whose handling injects a script into the active page — after
 *  a SUCCESSFUL one we arm the auto-track submit watcher (opt-in gated,
 *  idempotent per page). `stampResults` is deliberately EXCLUDED even though
 *  it injects a script: it is read-only (annotates a results page with
 *  saved/applied markers, no form interaction), so arming the watcher on a
 *  results page would let a later, unrelated submit-like interaction there
 *  auto-mark a saved application as applied (PR review finding). */
const GESTURE_KINDS: ReadonlySet<PopupRequest['kind']> = new Set([
  'import',
  'fill',
  'answersSave',
  'answersSuggest',
  'answerFill',
  'answerReplace',
  'answerScan',
  'answerAccept',
  'answerRestoreOriginal',
  'matchLive',
  'documentAttach',
]);

/** Client-side cap on the number of scanned question labels sent in one
 *  `answers.suggest` call — the desktop re-clamps independently (untrusted
 *  page-derived input), this just avoids sending an unbounded payload. */
const MAX_SUGGEST_QUESTIONS = 50;

/** Minimal guard for the `{question, index}[]` array that crossed the
 *  `executeScript` boundary (capture-questions.js's completion value). */
function isScannedQuestions(v: unknown): v is ScannedQuestion[] {
  return (
    Array.isArray(v) &&
    v.every(
      (e) =>
        typeof e === 'object' &&
        e !== null &&
        typeof (e as Record<string, unknown>).question === 'string' &&
        typeof (e as Record<string, unknown>).index === 'number'
    )
  );
}

/** Minimal guard for the fill outcome that crossed the `executeScript`
 *  boundary (answer-fill.js's completion value). */
function isFillAnswerResult(v: unknown): v is FillAnswerResult {
  if (typeof v !== 'object' || v === null) return false;
  return typeof (v as Record<string, unknown>).filled === 'boolean';
}

/** Minimal guard for the `{url, index}[]` array that crossed the
 *  `executeScript` boundary (`results-stamp.js`'s collect-step return, PR3). */
function isCollectedCards(v: unknown): v is CollectedCard[] {
  return (
    Array.isArray(v) &&
    v.every(
      (e) =>
        typeof e === 'object' &&
        e !== null &&
        typeof (e as Record<string, unknown>).url === 'string' &&
        typeof (e as Record<string, unknown>).index === 'number'
    )
  );
}

/** Minimal guard for the summary that crossed the `executeScript` boundary. */
function isFillSummary(v: unknown): v is AutofillSummary {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return Array.isArray(o.filled) && typeof o.filledNothing === 'boolean';
}

/** Minimal guard for the `{question, answer}[]` array that crossed the
 *  `executeScript` boundary (capture.js's `answers` completion field). */
function isCapturedAnswers(v: unknown): v is CapturedAnswer[] {
  return (
    Array.isArray(v) &&
    v.every(
      (e) =>
        typeof e === 'object' &&
        e !== null &&
        typeof (e as Record<string, unknown>).question === 'string' &&
        typeof (e as Record<string, unknown>).answer === 'string'
    )
  );
}

/** Minimal guard for the `{question, index, answer}[]` array that crossed
 *  the `executeScript` boundary (capture.js's `filled` completion field —
 *  PR 11's rewrite-mode picker source). */
function isFilledFields(v: unknown): v is FilledField[] {
  return (
    Array.isArray(v) &&
    v.every(
      (e) =>
        typeof e === 'object' &&
        e !== null &&
        typeof (e as Record<string, unknown>).question === 'string' &&
        typeof (e as Record<string, unknown>).index === 'number' &&
        typeof (e as Record<string, unknown>).answer === 'string'
    )
  );
}

/** Minimal guard for `capture.js`'s full completion value — `{answers,
 *  filled}` (see `capture.ts`'s doc for why both ride the SAME injection). */
function isCaptureResult(v: unknown): v is { answers: CapturedAnswer[]; filled: FilledField[] } {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return isCapturedAnswers(o.answers) && isFilledFields(o.filled);
}

/** Lazily-built, worker-lifetime-scoped client. Recreated after eviction. */
let client: BridgeClient | null = null;

function getClient(): BridgeClient {
  if (!client) {
    client = new BridgeClient(
      () => {
        // Best-effort push so an open popup live-updates; ignore "no receiver".
        void broadcastStatus();
      },
      // Provide the stored token so the bridge can perform the auth handshake on connect.
      getToken,
      // The desktop rotated its pairing secret (Settings → "Regenerate", or a
      // factory reset) and told us over the authenticated session. Un-pair
      // through the SAME path the popup's "Unpair" button uses, so a
      // desktop-initiated and a user-initiated un-pair can't drift.
      unpairLocally
    );
  }
  return client;
}

/**
 * Drop the stored pairing token and clear any auth block, leaving the bridge
 * ready to pair again. Shared by the popup's "Unpair" (`clearToken`) and the
 * desktop-initiated `token.revoked` frame.
 */
async function unpairLocally(): Promise<void> {
  await clearToken();
  getClient().resetForNewToken();
}

/** Fold raw bridge phase + token presence into the popup-facing status. */
async function computeStatus(): Promise<ConnectionStatus> {
  const hasToken = (await getToken()) !== null;
  const bridge = getClient().status();

  let phase: ConnectionStatus['phase'];
  if (bridge.phase === 'bad_token') {
    phase = 'bad_token';
  } else if (bridge.phase === 'outdated') {
    // Desktop too old for the v2 handshake → prompt the user to update the app.
    phase = 'outdated';
  } else if (bridge.phase === 'app_not_running') {
    phase = 'app_not_running';
  } else if (bridge.phase === 'searching') {
    phase = 'searching';
  } else if (!hasToken) {
    // Bridge reachable but we have no secret yet → show the pairing screen.
    phase = 'not_paired';
  } else {
    // bridge.phase === 'connected' AND hasToken → the mutual handshake succeeded.
    phase = 'connected';
  }
  return { phase, port: bridge.port, hasToken };
}

/** Push the current status to any listening popup (no-op if none is open). */
async function broadcastStatus(): Promise<void> {
  try {
    const status = await computeStatus();
    const message: PopupResponse = { ok: true, kind: 'status', status };
    await browser.runtime.sendMessage(message);
  } catch {
    // No popup open / port closed — fine.
  }
}

/** Push "a tracked application just flipped to applied" to any listening
 *  side panel (no-op if none is open). Sent by auto-track ONLY on a
 *  confirmed `saved → applied` write — see `PopupResponse`'s
 *  `jobStatusChanged` doc. Mirrors {@link broadcastStatus}'s try/catch and
 *  its use of the shared send channel: a panel that closed mid-flight, or
 *  one still on the content page with no listener, is never an error. */
async function broadcastJobStatusChanged(url: string): Promise<void> {
  try {
    const message: PopupResponse = { ok: true, kind: 'jobStatusChanged', url };
    await browser.runtime.sendMessage(message);
  } catch {
    // No panel open / port closed — fine.
  }
}

/** Append `delta` to `text`, clamped to the shared
 *  {@link EXTENSION_ANSWER_ASSIST_MAX_CHARS} cap — the same cap the Rust
 *  `answer_assist::DRAFT_CAP` mirrors. The desktop already clamps each
 *  `assist.chunk` live so this should never actually trip in normal
 *  operation; it exists so the interrupted/error path (which shows whatever
 *  text had already accumulated) can never render an unbounded draft even if
 *  that server-side guarantee were ever violated. */
function growAssistDraft(text: string, delta: string): string {
  const grown = text + delta;
  return grown.length > EXTENSION_ANSWER_ASSIST_MAX_CHARS
    ? grown.slice(0, EXTENSION_ANSWER_ASSIST_MAX_CHARS)
    : grown;
}

/**
 * The CURRENT (or last-finished) streaming `answer.assist` buffer — owned
 * HERE, not by the popup, so a popup that closes mid-stream and reopens can
 * immediately see what already arrived (see `PopupResponse`'s
 * `answerAssistProgress` doc). Single-slot: mirrors the popup's own "one
 * assist request at a time" UI (the button is disabled while one is in
 * flight) — a new `runAnswerAssist` call always resets it. `interrupted`
 * is set only when the stream ended in failure AFTER some text had already
 * accumulated (a clean "opt-in off"/"no provider" refusal before any text
 * arrives is a normal error, not an interruption).
 *
 * The "button disabled while in flight" UI invariant alone is NOT enough to
 * keep this single slot safe: an MV3 popup is torn down on close, so a popup
 * that closes mid-stream and reopens shows a fresh, enabled button and can
 * re-trigger `runAnswerAssist` while the first run is still in flight.
 * {@link assistGeneration} is what makes overlap safe — see `runAnswerAssist`.
 */
let assistBuffer: {
  text: string;
  done: boolean;
  interrupted: boolean;
  /** Which answer ROW this stream belongs to (ADR-044 decision 1), or `''`
   *  when the caller has no row model. The generation guard below is what
   *  keeps the single slot safe; the row id is what lets BOTH surfaces render
   *  the stream against the right question instead of against whatever was
   *  asked for last. It rides on the buffer rather than beside it so a
   *  superseding run replaces the text and its owner atomically. */
  rowId: string;
  /** `draft` (grounded, Regenerate) vs `rewrite` (reshape of the previous
   *  version). The UI says which is running rather than implying they are the
   *  same dial. */
  kind: 'draft' | 'rewrite';
  /** Present ONLY for a Prep tab on-demand draft (PR4) — a caller with no row
   *  model (empty `rowId`) tags its stream by `topic` instead. See
   *  `lib/answer-state.ts`'s `AnswerStream.topic` doc. */
  topic: ExtensionAnswerAssistRequest['topic'] | null;
} = {
  text: '',
  done: true,
  interrupted: false,
  rowId: '',
  kind: 'draft',
  topic: null,
};

/**
 * The tab whose shared answer state {@link assistBuffer} is mirrored into.
 * Captured with the row id when a run starts, because the mirror has to reach
 * the SAME `storage.session` record both views are subscribed to and the
 * active tab can change under a long stream.
 */
let assistTabId: number | null = null;

/**
 * Mirror the current assist buffer into the shared answer state, so a popup
 * that is closed (or was never open) and a panel on another surface both see
 * the same stream. Best-effort by construction: the buffer is still the
 * authority for the popup's own reattach query, and a failed session write
 * must never fail the user's click.
 */
async function mirrorAssistToState(): Promise<void> {
  const tabId = assistTabId;
  // A caller with a row model tags by `rowId`; a Prep tab draft (PR4, no row
  // model) tags by `topic` instead — either is enough to mirror, neither
  // alone (a buffer with NEITHER belongs to no view and is never mirrored).
  if (tabId === null || (!assistBuffer.rowId && !assistBuffer.topic)) return;
  const snapshot = { ...assistBuffer };
  await updateAnswerState(tabId, (state) => ({
    ...state,
    stream: {
      rowId: snapshot.rowId,
      text: snapshot.text,
      done: snapshot.done,
      interrupted: snapshot.interrupted,
      kind: snapshot.kind,
      ...(snapshot.topic ? { topic: snapshot.topic } : {}),
    },
  }));
}

/**
 * Single-flight generation counter for {@link assistBuffer}. `runAnswerAssist`
 * captures its own value on entry, superseding any prior run; a run whose
 * captured value no longer matches this counter has been superseded by a
 * newer overlapping call and must skip every `assistBuffer` write (chunks
 * and the terminal write alike) — see `runAnswerAssist`.
 */
let assistGeneration = 0;

/** Push the current assist buffer to any listening popup — mirrors
 *  `broadcastStatus` (no-op, silently, if none is open). */
async function broadcastAssistProgress(): Promise<void> {
  void mirrorAssistToState();
  try {
    const message: PopupResponse = {
      ok: true,
      kind: 'answerAssistProgress',
      text: assistBuffer.text,
      done: assistBuffer.done,
      interrupted: assistBuffer.interrupted,
      rowId: assistBuffer.rowId,
    };
    await browser.runtime.sendMessage(message);
  } catch {
    // No popup/panel open — fine.
  }
}

/**
 * The active tab of the window a request came from — the single seam every
 * "act on the current tab" lookup in this worker goes through.
 *
 * A service worker has NO window of its own, so `currentWindow: true` here
 * resolves to whichever window the browser focused last, not the window whose
 * popup or side panel sent the request. With a second window focused that is a
 * different tab entirely: a read failed with a confusing error, and Import
 * silently created an application from an unrelated page while reporting
 * success (#1215). Surfaces therefore send their own `windowId`
 * ({@link PopupRequest}), and this targets it.
 *
 * `undefined` keeps the old last-focused-window behaviour, which is the only
 * thing available when no window is known (an older surface build, or a flow
 * with no originating window at all). Anything acting on a resolved tab id
 * afterwards must keep using THAT id rather than re-querying — see
 * `captureTabHtml`'s doc.
 */
async function activeTabIn(windowId?: number): Promise<Browser.tabs.Tab | undefined> {
  const query =
    typeof windowId === 'number'
      ? { active: true, windowId }
      : { active: true, currentWindow: true };
  const [tab] = await browser.tabs.query(query);
  return tab;
}

/** Resolve the active tab's URL for an import. */
async function activeTabUrl(windowId?: number): Promise<string> {
  const tab = await activeTabIn(windowId);
  const url = tab?.url ?? '';
  if (!url) throw new Error('Could not read the current tab URL.');
  return url;
}

/**
 * Scan mode: inject the capture script into the active tab and return its
 * `outerHTML`. Requires `scripting` + `activeTab` (granted on the click).
 */
async function captureActiveTabHtml(windowId?: number): Promise<string> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to scan.');
  return captureTabHtml(tabId);
}

/**
 * Same capture as {@link captureActiveTabHtml}, but against an ALREADY
 * RESOLVED `tabId` instead of re-querying "the active tab" — used by callers
 * (like `runMatchLive`) that must keep acting on the one tab they resolved at
 * the start of a multi-step, possibly slow flow, not whichever tab happens to
 * be active by the time this step runs.
 */
async function captureTabHtml(tabId: number): Promise<string> {
  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['content.js'],
  });
  const html = results[0]?.result;
  if (typeof html !== 'string' || html.length === 0) {
    throw new Error('Could not capture the page DOM.');
  }
  return html;
}

/** Run an import, always attempting to capture the rendered DOM first. */
async function runImport(applied: boolean, windowId?: number): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl(windowId);
  const payload: ExtensionImportRequest = { url, applied };
  // Always try to capture the authenticated DOM so the desktop can parse it
  // without re-fetching (which would hit bot-walls on LinkedIn/Indeed/Glassdoor).
  // Fall back to URL-only if executeScript is blocked (restricted pages).
  try {
    payload.html = await captureActiveTabHtml(windowId);
  } catch {
    // ponytail: restricted page or scripting permission denied — URL-only fallback
  }

  const result = await getClient().importJob(payload);
  return { ok: true, kind: 'import', result };
}

/**
 * Inject the assisted-autofill filler into the active tab and run it with the
 * given profile. Two steps so the profile (PII) is passed transiently as an
 * `executeScript` arg rather than through any stored/registered surface:
 *   1. `files: ['fill.js']` registers {@link runAutofill} on the page global;
 *   2. a self-contained `func` (params + `globalThis` only) calls it with the
 *      profile and returns the summary.
 */
async function injectFill(profile: AutofillProfile, windowId?: number): Promise<AutofillSummary> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to fill.');

  await browser.scripting.executeScript({ target: { tabId }, files: ['fill.js'] });

  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (p: AutofillProfile, key: string): AutofillSummary | null => {
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((profile: AutofillProfile) => AutofillSummary) | undefined;
      return runner ? runner(p) : null;
    },
    args: [profile, AUTOFILL_GLOBAL],
  });

  const summary = results[0]?.result;
  if (!isFillSummary(summary)) {
    throw new Error('Could not fill the form on this page.');
  }
  return summary;
}

/**
 * Assisted autofill: fetch the contact profile FRESH from the desktop (gated by
 * the desktop's opt-in — a refusal surfaces as an error) and inject the filler.
 * The profile is held only for this call and never persisted client-side.
 */
async function runFill(windowId?: number): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const profile = await getClient().getProfile();
  if (profile.error) {
    // Desktop refused (autofill off) or the reply was malformed — surface it.
    return { ok: false, error: profile.error };
  }

  // Project to the fill shape, dropping the transport-only `error` field.
  const fields: AutofillProfile = {
    fullName: profile.fullName,
    email: profile.email,
    phone: profile.phone,
    location: profile.location,
    linkedin: profile.linkedin,
    github: profile.github,
    website: profile.website,
    extraLinks: profile.extraLinks,
  };
  const summary = await injectFill(fields, windowId);
  return { ok: true, kind: 'fill', summary };
}

/**
 * Job tab copy-field fallback (decision 8): fetch the Contact Profile fresh —
 * same source + Autofill opt-in gate `runFill` itself uses — for the panel to
 * show as Copy-able fields when a `fill` came back `filledNothing`. Passive:
 * NEVER surfaces `ok:false`. Any failure (not paired, bridge down, opt-in
 * off, a malformed reply) folds into `result.error` so the caller shows
 * nothing rather than an error, mirroring `runAppliedCheck`'s fail-quiet
 * discipline. The profile is held only for this call, never persisted.
 */
async function runProfileGet(): Promise<PopupResponse> {
  try {
    const token = await getToken();
    if (!token) {
      return { ok: true, kind: 'profileGet', result: { error: 'Not paired.' } };
    }
    const result = await getClient().getProfile();
    return { ok: true, kind: 'profileGet', result };
  } catch (err) {
    return {
      ok: true,
      kind: 'profileGet',
      result: { error: err instanceof Error ? err.message : String(err) },
    };
  }
}

/**
 * Fire-and-forget "have I already applied?" check for the active tab's URL —
 * a read-only, best-effort enhancement over the import view. NEVER surfaces
 * `ok:false`: any failure (not paired, bridge unreachable, an old desktop's
 * unrecognized message type, a malformed reply) folds into `{ found: false }`
 * so the popup renders nothing rather than an error.
 */
async function runAppliedCheck(windowId?: number): Promise<PopupResponse> {
  try {
    const url = await activeTabUrl(windowId);
    const result = await getClient().checkApplied(url);
    return { ok: true, kind: 'appliedCheck', result };
  } catch {
    return { ok: true, kind: 'appliedCheck', result: { found: false } };
  }
}

/** The `probe-fields.js` completion value — see that file's doc for why two
 *  independent booleans, not one. */
interface FieldsProbeResult {
  hasFormFields: boolean;
  hasAnswerFields: boolean;
}

/** Minimal guard for the `{hasFormFields, hasAnswerFields}` object that
 *  crossed the `executeScript` boundary (probe-fields.js's completion value). */
function isFieldsProbeResult(v: unknown): v is FieldsProbeResult {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return typeof o.hasFormFields === 'boolean' && typeof o.hasAnswerFields === 'boolean';
}

/**
 * Inject the fillable-fields probe into the active tab and return its
 * `{hasFormFields, hasAnswerFields}` completion value. Single-step injection
 * — same pattern as `captureActiveTabQuestions`. UNLIKE the other capture
 * injections, this never needs a token check first: it never touches the
 * bridge/desktop at all, only the active tab's DOM.
 */
async function captureActiveTabFieldsProbe(windowId?: number): Promise<FieldsProbeResult> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to scan.');

  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['probe-fields.js'],
  });
  const probe = results[0]?.result;
  if (!isFieldsProbeResult(probe)) throw new Error('Could not scan this page.');
  return probe;
}

/**
 * Passive "does this page have fillable form fields?" probe, run once when
 * the popup shows the connected view — gates the Form group / Answer-tools
 * disclosure. Mirrors `runAppliedCheck`'s never-surfaces-`ok:false` fold, but
 * FAILS OPEN instead of closed: any failure (no active tab, restricted page,
 * scripting permission denied) resolves BOTH signals `true` so a probe bug
 * can never hide either feature — only a CONFIRMED empty scan hides them.
 */
async function runFieldsProbe(windowId?: number): Promise<PopupResponse> {
  try {
    const { hasFormFields, hasAnswerFields } = await captureActiveTabFieldsProbe(windowId);
    return { ok: true, kind: 'fieldsProbe', hasFormFields, hasAnswerFields };
  } catch {
    return { ok: true, kind: 'fieldsProbe', hasFormFields: true, hasAnswerFields: true };
  }
}

/**
 * Passive "is assisted autofill on?" read (Task #30), run once when the
 * popup shows the connected view — the popup uses it to decide whether to
 * auto-run "Suggest answers for this form" without a click. Mirrors
 * `runFieldsProbe`'s always-`ok:true` fold: `autofillEnabled()` itself never
 * rejects (any failure degrades to `false`, the safe default — see its
 * doc), so this never needs its own catch.
 */
async function runAutofillCheck(): Promise<PopupResponse> {
  const enabled = await getClient().autofillEnabled();
  return { ok: true, kind: 'autofillCheck', enabled };
}

/** Minimal guard for the `job` resource's `data` shape (PR1 — extension read
 *  tier) — only the two fields the trust line renders; every other field
 *  the resource carries is ignored here. `title`/`company` are optional
 *  strings on the wire (a stub/partial posting may lack either). */
function readJobTitleCompany(data: unknown): { title: string | null; company: string | null } {
  if (typeof data !== 'object' || data === null) return { title: null, company: null };
  const o = data as Record<string, unknown>;
  // The Rust BE fences `title`/`company` on every curated read
  // (`fence_posting_display_fields`, tag `job_posting` — see
  // `extension_bridge/agent_read/best_matches.rs`), so undo the exact
  // wrapper BEFORE the trim-guard: the trust line must render
  // "Senior Engineer", not the literal `<job_posting>…</job_posting>`
  // markup, and an empty wrapped value must degrade to `null` just like
  // a missing one.
  const title = typeof o.title === 'string' ? stripFenceWrapper('job_posting', o.title) : null;
  const company =
    typeof o.company === 'string' ? stripFenceWrapper('job_posting', o.company) : null;
  return {
    title: title && title.trim() ? title : null,
    company: company && company.trim() ? company : null,
  };
}

/**
 * Passive "what does the read tier say about this page's job?" lookup (PR1
 * — extension read tier), feeding `sidepanel.ts`'s trust line. Mirrors
 * `runAppliedCheck`'s always-`ok:true`, never-throws fold: ANY refusal
 * (Autofill off, throttled, an unknown job, no connection) resolves both
 * fields `null`, which the panel renders as its existing host-only line.
 */
async function runTrustLineJob(windowId?: number): Promise<PopupResponse> {
  try {
    const url = await activeTabUrl(windowId);
    const res = await getClient().agentQuery('job', { url });
    if (!res.ok) return { ok: true, kind: 'trustLineJob', title: null, company: null };
    return { ok: true, kind: 'trustLineJob', ...readJobTitleCompany(res.data) };
  } catch {
    return { ok: true, kind: 'trustLineJob', title: null, company: null };
  }
}

/**
 * Settings page: read the extension's opt-in switches (R7 of the redesign
 * record). UNLIKE `runAutofillCheck`, failures are NOT folded away — a
 * transport-level rejection propagates to `handleRequest`'s outer catch,
 * and a resolved desktop-side refusal passes straight through as `result`
 * so the page can show why the toggles couldn't load.
 */
async function runSettingsGet(): Promise<PopupResponse> {
  const result = await getClient().settingsGet();
  return { ok: true, kind: 'settingsGet', result };
}

/**
 * Settings page: flip one switch. Like `runStatusUpdate`, failures are NOT
 * folded away — the page rolls its optimistic toggle back on a well-formed
 * `ok:false`.
 */
async function runSettingsSet(key: ExtensionSettingsKey, enabled: boolean): Promise<PopupResponse> {
  const result = await getClient().settingsSet(key, enabled);
  return { ok: true, kind: 'settingsSet', result };
}

/**
 * User-clicked "Mark as applied" for the active tab's URL. UNLIKE
 * `runAppliedCheck`, failures are NOT folded away here: a transport-level
 * rejection (not paired, bridge unreachable, timeout) propagates up to
 * `handleRequest`'s outer catch as `{ ok: false, error }`, and a resolved
 * desktop-side refusal (`{ ok: false, error }` — no match / wrong starting
 * status / unsupported transition) still passes straight through as `result`
 * — this is a deliberate click action, so the user must see why it failed.
 */
async function runStatusUpdate(windowId?: number): Promise<PopupResponse> {
  const url = await activeTabUrl(windowId);
  const result = await getClient().updateStatus(url);
  return { ok: true, kind: 'statusUpdate', result };
}

// ── Documents into ATS (PR2) ────────────────────────────────────────────────

/** Minimal guard for the summary that crossed the `executeScript` boundary
 *  (attach-file.js's completion value). Mirrors `isFillAnswerResult`. */
function isAttachFileResult(v: unknown): v is AttachFileResult {
  if (typeof v !== 'object' || v === null) return false;
  return typeof (v as Record<string, unknown>).attached === 'boolean';
}

/**
 * Decode a base64 `document.result` payload to raw bytes. Shared by the
 * attach/paste document flows so `bridge.ts` stays base64-agnostic (per its
 * own doc, it decodes NOTHING beyond checking `dataEncoding === 'base64'`) —
 * this is the one place that turns the wire string into bytes.
 */
function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * Documents tab (PR2 §C.2): list this job's generation + saved base résumés
 * — the curated `documents` read-tier resource (PR1, `agent.query`). UNLIKE
 * `runTrustLineJob`, a refusal is NOT folded away — the tab renders the
 * desktop's own `error` (Autofill opt-in off / throttled) so the user can
 * act on it. `url` is echoed back so the Documents tab (which has no `tabs`
 * permission of its own) can build the `{kind:'generation', url}` source for
 * the job's own generation candidate without a second round trip.
 */
async function runDocumentsList(windowId?: number): Promise<PopupResponse> {
  const url = await activeTabUrl(windowId).catch(() => '');
  const result = await getClient().agentQuery('documents', { url });
  return { ok: true, kind: 'documentsList', result, url };
}

// ── Prep tab (PR4) ───────────────────────────────────────────────────────────

/**
 * Prep tab: read this job's existing generations (company brief, interview
 * questions, salary answer) — the curated `prep` read-tier resource (PR1,
 * `agent.query`). Same shape as {@link runDocumentsList}: a refusal is NOT
 * folded away (the tab renders the desktop's own `error`), and `url` is
 * echoed back so the tab (no `tabs` permission of its own) can build the
 * "Prepare in the app" deep link without a second round trip.
 */
async function runPrepGet(windowId?: number): Promise<PopupResponse> {
  const url = await activeTabUrl(windowId).catch(() => '');
  const result = await getClient().agentQuery('prep', { url });
  return { ok: true, kind: 'prepGet', result, url };
}

/**
 * Cancel whatever `answer.assist` stream is currently pending (PR4 — the
 * Prep tab's Cancel button). Always `ok:true` — a no-op when nothing was
 * pending is not an error, mirrors {@link BridgeClient.cancelCurrent}'s doc.
 */
function runAssistCancel(): PopupResponse {
  getClient().cancelCurrent();
  return { ok: true, kind: 'assistCancel' };
}

/**
 * Fire-and-forget "was there a transparent save-answers-on-submit notice
 * waiting for me?" (PR4) — read-once, see `lib/auto-save-notice.ts`'s doc.
 */
async function runAutoSaveNotice(): Promise<PopupResponse> {
  const text = await takeAutoSaveNotice();
  return { ok: true, kind: 'autoSaveNotice', text };
}

/**
 * Documents tab: export the picked source as DECODED plain text (cover
 * letter, TXT only — the picker's Copy/Paste actions both need text, never
 * base64). Like `runStatusUpdate`, failures are NOT folded away — a
 * deliberate click.
 */
async function runDocumentExportText(
  source: ExtensionDocumentSource,
  templateId: string,
  letterLayoutId: string | undefined
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }
  const res = await getClient().documentExport({
    source,
    kind: 'cover-letter',
    format: 'txt',
    templateId,
    ...(letterLayoutId ? { letterLayoutId } : {}),
  });
  if (!res.ok) return { ok: false, error: res.error };
  const text = new TextDecoder().decode(base64ToBytes(res.data));
  return { ok: true, kind: 'documentExportText', text, filename: res.filename };
}

/**
 * Inject the résumé-attach script into `tabId` and run it against `base64`.
 * Two-step like `injectFill`: the payload (the user's own résumé) is passed
 * in transiently via the second `executeScript({ func, args })` rather than
 * baked into the `files` injection.
 *
 * The payload crosses that boundary as a base64 STRING, never a `Uint8Array`
 * (PR review round 2 — a real defect, not a hypothetical): Chrome
 * JSON-serializes `executeScript` `args`, so a `Uint8Array` arrives in the
 * injected function as a plain `{"0":…,"1":…}` object — `new
 * Uint8Array(that)` is empty and `that.byteLength` is `undefined`, so
 * `attachResumeFile`'s own byte-length verification then always refuses. A
 * base64 string is JSON-safe and survives the boundary intact; `attach-
 * file.ts`'s injected {@link runAttachFile} decodes it back to bytes INSIDE
 * the page (see that function's own doc).
 */
async function injectAttachFile(
  tabId: number,
  base64: string,
  filename: string,
  mimeType: string
): Promise<AttachFileResult> {
  await browser.scripting.executeScript({ target: { tabId }, files: ['attach-file.js'] });

  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (data: string, name: string, mime: string, key: string): AttachFileResult | null => {
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((base64: string, filename: string, mimeType: string) => AttachFileResult) | undefined;
      return runner ? runner(data, name, mime) : null;
    },
    args: [base64, filename, mimeType, ATTACH_FILE_GLOBAL],
  });

  const result = results[0]?.result;
  if (!isAttachFileResult(result)) {
    throw new Error('Could not attach the file on this page.');
  }
  return result;
}

/**
 * Re-verify, right before injection, that `tabId` is still the active tab
 * AND still on `origin` (PR review round 2). `runDocumentAttach` captures
 * both BEFORE the desktop export round trip below, which can take long
 * enough for the user to switch tabs or navigate away — without this check
 * the résumé would attach to whatever page happens to be active once the
 * export finally resolves, not the one the user confirmed.
 */
async function tabStillConfirmed(
  tabId: number,
  origin: string,
  windowId?: number
): Promise<boolean> {
  const tab = await activeTabIn(windowId);
  if (tab?.id !== tabId || !tab.url) return false;
  try {
    return new URL(tab.url).origin === origin;
  } catch {
    return false;
  }
}

/**
 * Same defect class as {@link tabStillConfirmed}, checked at EXACT-url
 * granularity rather than origin: `runMatchLive` binds its badge to the tab
 * + url it resolved before the (possibly slow) desktop round trip, and must
 * re-verify both are unchanged right before painting the badge — a tab
 * switch, or a same-tab navigation to a different posting on the same
 * origin, must not paint one page's score onto another (PR review finding).
 */
async function tabStillOnExactUrl(tabId: number, url: string, windowId?: number): Promise<boolean> {
  const tab = await activeTabIn(windowId);
  return tab?.id === tabId && tab.url === url;
}

/**
 * "Attach résumé to this page": export as pdf/docx, inject via {@link
 * injectAttachFile}, and surface the fail-closed outcome. The caller
 * (`documents/documents.ts`) is responsible for the first-time-per-site
 * confirmation BEFORE sending this request. Like `runStatusUpdate`, failures
 * are NOT folded away — a deliberate click.
 */
async function runDocumentAttach(
  source: ExtensionDocumentSource,
  templateId: string,
  format: 'pdf' | 'docx',
  windowId?: number
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }
  // Bind the attach to the tab + origin confirmed BEFORE the (possibly slow)
  // desktop export round trip — re-verified via `tabStillConfirmed` right
  // before injection (PR review round 2).
  const tabId = await activeTabId(windowId);
  const origin = await activeTabOriginAtGesture(windowId);
  const res = await getClient().documentExport({ source, kind: 'resume', format, templateId });
  if (!res.ok) return { ok: false, error: res.error };
  if (!(await tabStillConfirmed(tabId, origin, windowId))) {
    return { ok: false, error: 'The page changed while exporting — please retry.' };
  }
  const result = await injectAttachFile(tabId, res.data, res.filename, res.mimeType);
  return { ok: true, kind: 'documentAttach', result };
}

// ── Results-page stamps (PR3 §B.4) ──────────────────────────────────────────────

/**
 * Step one: inject the results-stamp entry (registers BOTH the collect and
 * stamp globals on the page — see `results-stamp.ts`'s doc) and call the
 * collector. Returns the candidate `{url, index}[]` the background sends on
 * as `applied.check.batch`.
 */
async function injectResultsCollect(tabId: number): Promise<CollectedCard[]> {
  await browser.scripting.executeScript({ target: { tabId }, files: ['results-stamp.js'] });
  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (key: string): unknown => {
      const runner = (globalThis as Record<string, unknown>)[key] as (() => unknown) | undefined;
      return runner ? runner() : null;
    },
    args: [RESULTS_COLLECT_GLOBAL],
  });
  const collected = results[0]?.result;
  if (!isCollectedCards(collected)) throw new Error('Could not read job cards on this page.');
  return collected;
}

/**
 * Step two: call the SAME injected instance's stamper with the resolved
 * `applied.check.batch` entries (in the SAME order the urls were sent —
 * `stampResultsCards` maps them back to its own collected anchors by
 * index). Returns how many cards were actually stamped.
 */
async function injectResultsStamp(tabId: number, entries: StampInput[]): Promise<number> {
  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (data: StampInput[], key: string): unknown => {
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((r: StampInput[]) => number) | undefined;
      return runner ? runner(data) : 0;
    },
    args: [entries, RESULTS_STAMP_GLOBAL],
  });
  const stamped = results[0]?.result;
  return typeof stamped === 'number' ? stamped : 0;
}

/**
 * User-clicked "Stamp this results page" (PR3 §B.4). Mirrors `runAnswersSave`'s
 * not-paired short-circuit. UNLIKE most gesture verbs, a refusal at any step
 * beyond "not paired" (over-cap, throttled, a malformed batch reply, a
 * collect/stamp injection failure) degrades to a `stamped: 0` + explanatory
 * `status` line rather than `ok:false` — "respect the refusals… degrade to
 * no stamps, never a partial lie" (PR3 §B.4). Restricted pages are caught
 * EARLY by {@link isPermanentlyUnreadablePage} (or a redacted empty url, no
 * `tabs` permission) and answered with the shared {@link UNREADABLE_PAGE_MSG}
 * — never `ok:false` from the reload hint, which would be a lie on a page
 * that can never be read; the shared {@link CAPTURE_FAILED_MSG} reload hint
 * stays reserved for genuinely transient failures on readable pages (#1219).
 */
async function runStampResults(windowId?: number): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }
  // Re-read the preference here too (never cached at load) — defense in
  // depth against a stale UI state that still shows the button after the
  // user turned the preference off elsewhere (Settings page, another
  // surface).
  if (!(await getStampResultsPages())) {
    return { ok: false, error: 'Results-page stamps are off. Turn them on in Settings.' };
  }

  // Resolve the active tab ONCE, capturing its url too — the same snapshot
  // discipline as `runMatchLive`: everything below (the collect injection, the
  // batch check, the stamp injection) must target the SAME tab this gesture
  // started on, not "whatever is active" at each await point.
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  const tabUrl = tab?.url ?? '';
  // No tab at all, a redacted url (no `tabs` permission), or a readable-but-
  // restricted kind are all permanent: there is nothing to stamp and a reload
  // hint would be a lie, so both fold into the shared unreadable-page message
  // exactly like `runMatchLive`.
  if (typeof tabId !== 'number' || tabUrl === '' || isPermanentlyUnreadablePage(tabUrl)) {
    return { ok: false, error: UNREADABLE_PAGE_MSG };
  }

  let collected: CollectedCard[];
  try {
    collected = await injectResultsCollect(tabId);
  } catch {
    // Genuinely transient failure on a readable page — the reload hint is
    // truthful here, so reuse the shared capture-failed message (never the
    // unreadable-page one, which would be a lie on a page we CAN reach).
    return { ok: false, error: CAPTURE_FAILED_MSG };
  }
  if (collected.length === 0) {
    return {
      ok: true,
      kind: 'stampResults',
      stamped: 0,
      status: 'No job cards found on this page.',
    };
  }

  let batch: Awaited<ReturnType<BridgeClient['checkAppliedBatch']>>;
  try {
    batch = await getClient().checkAppliedBatch(collected.map((c) => c.url));
  } catch {
    return {
      ok: true,
      kind: 'stampResults',
      stamped: 0,
      status: 'Could not reach the desktop app.',
    };
  }
  if (!batch.ok) {
    return { ok: true, kind: 'stampResults', stamped: 0, status: batch.error };
  }

  const entries: StampInput[] = batch.results.map((r) => {
    const out: StampInput = { url: r.url, found: r.found };
    if (r.status !== undefined) out.status = r.status;
    return out;
  });

  let stamped: number;
  try {
    stamped = await injectResultsStamp(tabId, entries);
  } catch {
    return { ok: true, kind: 'stampResults', stamped: 0, status: 'Could not stamp this page.' };
  }

  return {
    ok: true,
    kind: 'stampResults',
    stamped,
    status:
      stamped > 0
        ? `Stamped ${stamped} card${stamped === 1 ? '' : 's'}.`
        : 'No saved/applied jobs found among the cards on this page.',
  };
}

// ── Auto-track (Task #22, Layer A) ──────────────────────────────────────────────

/** Guard for the injected submit-watcher's fire-and-forget message. `answers`
 *  (PR4) is present only when the watcher was armed with `captureAnswers:
 *  true` AND something was filled — validated with the SAME guard
 *  `capture.js`'s completion value uses. */
function isSubmitDetected(
  v: unknown
): v is { kind: 'submitDetected'; url: string; answers?: CapturedAnswer[] } {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  if (o.kind !== SUBMIT_DETECTED_MSG || typeof o.url !== 'string') return false;
  return o.answers === undefined || isCapturedAnswers(o.answers);
}

/** Guard for the fit badge's fire-and-forget "Open the panel" click (PR3) —
 *  mirrors {@link isSubmitDetected}'s shape exactly. */
function isOpenPanelFromBadge(v: unknown): v is { kind: typeof OPEN_PANEL_FROM_BADGE_MSG } {
  if (typeof v !== 'object' || v === null) return false;
  return (v as Record<string, unknown>).kind === OPEN_PANEL_FROM_BADGE_MSG;
}

/**
 * Inject the auto-track submit watcher into the active tab. Two-step (PR4),
 * like `injectFill`: `files` registers {@link SUBMIT_WATCH_GLOBAL} on the
 * page, then a self-contained `func` arms it with `captureAnswers` — the
 * desktop-enforced `saveAnswersOnSubmit` opt-in value, resolved by
 * {@link maybeArmSubmitWatch} BEFORE this call — passed as a plain
 * JSON-safe boolean (only JSON-safe primitives ever cross this boundary).
 * Called only after a successful gesture + only when the auto-track opt-in
 * is on; the watcher's own isolated-world flag makes a repeat injection on
 * the same page a no-op regardless of the flag value on that later call.
 */
async function injectSubmitWatch(captureAnswers: boolean, windowId?: number): Promise<void> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') return;
  await browser.scripting.executeScript({ target: { tabId }, files: ['submit-watch.js'] });
  await browser.scripting.executeScript({
    target: { tabId },
    func: (capture: boolean, key: string): void => {
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((c: boolean) => void) | undefined;
      runner?.(capture);
    },
    args: [captureAnswers, SUBMIT_WATCH_GLOBAL],
  });
}

/**
 * Nudge the user (action badge) that they submitted an application for a job the
 * app isn't tracking — clicking the extension action opens the popup, whose
 * existing Import button captures the page. Uses only the always-available
 * `action` API — NO `notifications` permission (Task #22 adds none).
 */
function promptImport(): void {
  try {
    browser.action.setBadgeText({ text: '!' }).catch(() => {});
    browser.action.setBadgeBackgroundColor({ color: '#2563eb' }).catch(() => {});
  } catch {
    // action API unavailable — skip the nudge.
  }
}

/** Clear the untracked-submit nudge (called when the popup opens). */
function clearImportPrompt(): void {
  try {
    browser.action.setBadgeText({ text: '' }).catch(() => {});
  } catch {
    // ignore — nothing to clear.
  }
}

/**
 * Read the desktop-enforced save-answers-on-submit opt-in (PR4) — resolves
 * `true` only when `settings.get` replies with `saveAnswersOnSubmit: true`.
 * Mirrors `BridgeClient.autotrackEnabled`/`autofillEnabled` exactly: NEVER
 * rejects, any failure (not connected, a malformed reply) degrades to
 * `false` (OFF, the safe default) — the two callers (`maybeArmSubmitWatch`
 * via `submitFlowDeps`'s sibling call site) treat "unknown" exactly as "off".
 * Rides `settings.get` rather than a dedicated wire verb (decision: the
 * fourth switch is reachable ONLY through `settings.get`/`settings.set`,
 * never a bespoke read like `autotrackCheck`).
 */
async function saveAnswersOnSubmitEnabled(): Promise<boolean> {
  try {
    const res = await getClient().settingsGet();
    return res.ok && res.settings.saveAnswersOnSubmit === true;
  } catch {
    return false;
  }
}

/** Auto-track dependencies wired to the live bridge client. */
function submitFlowDeps() {
  return {
    autotrackEnabled: () => getClient().autotrackEnabled(),
    checkApplied: (url: string) => getClient().checkApplied(url),
    updateStatusAuto: (url: string) => getClient().updateStatus(url, true),
    promptImport,
    saveAnswersAuto: (url: string, answers: CapturedAnswer[]) =>
      getClient().saveAnswers(url, answers, true),
    notifyAutoSave: (result: Extract<ExtensionAnswersSaveResult, { ok: true }>) => {
      const count = result.saved;
      void setAutoSaveNotice(
        `Saved ${count} answer${count === 1 ? '' : 's'} from this submit${result.title ? ` (${result.title})` : ''} — change this in Settings → What the extension may do.`
      );
    },
    // The side panel's ONLY event-driven refresh: push the flipped
    // application's url so it re-reads just that job (never polls). Fires
    // inside `handleSubmitDetected` only on an `updateStatusAuto` ok:true.
    notifyJobStatusChanged: (url: string) => {
      void broadcastJobStatusChanged(url);
    },
  };
}

/**
 * Inject the answers-capture collector into the active tab and return its
 * `{answers, filled}` (see `capture.ts`'s doc). Single-step injection (unlike
 * `injectFill`'s files+func two-step): the collector takes no PII input to
 * pass in transiently, it only reads the page and returns data — same
 * pattern as `captureActiveTabHtml`.
 */
async function captureActiveTabFormData(windowId?: number): Promise<{
  answers: CapturedAnswer[];
  filled: FilledField[];
}> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to capture.');

  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['capture.js'],
  });
  const captured = results[0]?.result;
  if (!isCaptureResult(captured)) {
    throw new Error('Could not read the answers on this page.');
  }
  return captured;
}

/**
 * User-clicked "Save my answers from this page". UNLIKE `runAppliedCheck`,
 * failures are NOT folded away — a deliberate click action, like
 * `runStatusUpdate` — so a capture/transport failure propagates up to
 * `handleRequest`'s outer catch as `{ ok: false, error }`, and a resolved
 * desktop-side refusal (`{ ok: false, error }` — autofill off / no match /
 * malformed) still passes straight through as `result`. Mirrors `runFill`'s
 * not-paired short-circuit: the token check runs BEFORE the capture injection,
 * so an unpaired browser never reads the page. `filled` (PR 11) rides the
 * SAME capture so the popup can source its rewrite picker without a second
 * scan/injection.
 */
async function runAnswersSave(windowId?: number): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl(windowId);
  const { answers, filled } = await captureActiveTabFormData(windowId);
  const result = await getClient().saveAnswers(url, answers);
  return { ok: true, kind: 'answersSave', result, filled };
}

/**
 * Inject the questions-mode collector into the active tab and return its
 * `{question, index}[]` scan-time correlation list. Single-step injection —
 * same pattern as `captureActiveTabAnswers`.
 */
async function captureActiveTabQuestions(windowId?: number): Promise<ScannedQuestion[]> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to scan.');

  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['capture-questions.js'],
  });
  const questions = results[0]?.result;
  if (!isScannedQuestions(questions)) {
    throw new Error('Could not read the questions on this page.');
  }
  return questions;
}

/**
 * User-clicked "Suggest answers for this form". Mirrors `runAnswersSave`'s
 * not-paired short-circuit (token checked BEFORE the scan injection) and its
 * never-fold-errors discipline — a deliberate click, so failures propagate to
 * `handleRequest`'s outer catch, and a resolved desktop-side refusal passes
 * straight through as `result`. The scanned correlation list rides alongside
 * `result` so the popup can decide, per suggestion, whether a live Fill
 * target still exists on the page.
 */
async function runAnswersSuggest(windowId?: number): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const scanned = await captureActiveTabQuestions(windowId);
  // Dedup by exact text (the desktop dedups by normalized text) and cap
  // client-side — untrusted page content, never send an unbounded array.
  const questions = [...new Set(scanned.map((q) => q.question))].slice(0, MAX_SUGGEST_QUESTIONS);
  const result = await getClient().suggestAnswers(questions);
  return { ok: true, kind: 'answersSuggest', result, scanned };
}

/** Qualitative band next to the score — mirrors `job-tools.ts::scoreBand`
 *  EXACTLY (duplicated rather than imported: `job-tools.ts` is UI-mounting
 *  code with its own dependency chain, and this is a 3-line pure function,
 *  not a wire contract, so there is nothing to keep in protocol lockstep —
 *  same "duplicate the tiny bit" discipline as `AUTOFILL_GLOBAL` above). */
function fitBadgeScoreBand(score: number): FitBadgeView['band'] {
  if (score >= 80) return 'strong match';
  if (score >= 50) return 'partial match';
  return 'low match';
}

/** Mirrors `job-tools.ts`'s `SCORE_SOURCE_LABEL` EXACTLY — same duplication
 *  discipline as {@link fitBadgeScoreBand} above: this is a 2-entry literal
 *  map, not a wire contract. The on-page badge must show this qualifier too
 *  (never one tap deeper than the panel/popup card does). */
const FIT_BADGE_SCORE_SOURCE_LABEL: Record<'keyword' | 'combined', string> = {
  keyword: 'keyword coverage',
  combined: 'combined (keyword + semantic)',
};

/**
 * Inject the on-page fit badge into `tabId` and render `view`. Two steps so
 * the match result is passed transiently as an `executeScript` arg rather
 * than through any stored/registered surface — mirrors `injectFill`'s
 * two-step register-then-invoke pattern exactly. Every value on `view` is a
 * JSON-safe primitive/array/plain-object (PR2 lesson).
 *
 * `url` is the SAME url `runMatchLive` captured before the desktop round
 * trip (and that `tabStillOnExactUrl` already re-verified against `tabs.url`
 * just before this call). It is threaded through as a third, JSON-safe
 * string arg and re-checked ONE more time, IN the page, immediately before
 * the renderer runs — the last possible point, catching a navigation during
 * `maybeShowFitBadge`'s OWN later awaits (`getShowFitBadge`,
 * `checkApplied`), which land after that background-side check and so
 * aren't covered by it (PR review finding). A full navigation loads a fresh
 * document that this call re-injects `fit-badge.js` into; an SPA navigation
 * instead keeps the already-installed global alive on the SAME document
 * with a new `location.href`. Either way `location.href` is the page's own
 * live truth, so an exact match against the captured `url` — the same
 * strictness `tabStillOnExactUrl` already applies one step earlier, both
 * comparing the one full tab-url string end to end — is the only comparison
 * that can never let a different posting through; a page rewriting its own
 * `location.href` (an in-page fragment/route change) is exactly the
 * different-posting risk this check exists to catch, not a false positive
 * to relax away.
 *
 * The badge does not stop verifying here. The rendered badge keeps watching
 * `location.href` on a `STALE_URL_POLL_MS` poll plus popstate/hashchange (see
 * `lib/fit-badge.ts`'s {@link renderFitBadge}) and clears itself the moment
 * the page moves to a different url — the SPA job→job navigation (issue
 * #1221) that can happen AFTER this call, with the badge already on screen,
 * is what that in-badge watcher catches. The captured `expectedUrl` is what
 * the watcher compares against, so this seam stays the single source of
 * truth for "which posting is the badge about".
 */
async function injectFitBadge(tabId: number, url: string, view: FitBadgeView): Promise<void> {
  await browser.scripting.executeScript({ target: { tabId }, files: ['fit-badge.js'] });
  await browser.scripting.executeScript({
    target: { tabId },
    func: (v: FitBadgeView, key: string, expectedUrl: string): void => {
      if (location.href !== expectedUrl) return;
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((view: FitBadgeView, expectedUrl?: string) => void) | undefined;
      runner?.(v, expectedUrl);
    },
    args: [view, FIT_BADGE_GLOBAL, url],
  });
}

/**
 * Best-effort on-page badge after a successful Check-fit (PR3 §B.3) — ONLY
 * when `getShowFitBadge()` is true. Never affects the popup's own
 * `matchLive` response: every failure here (opt-in read, the applied-status
 * lookup, the injection itself) is swallowed, since this is a UI
 * enhancement layered on an already-successful gesture, not the gesture's
 * own result.
 */
async function maybeShowFitBadge(
  tabId: number,
  url: string,
  result: Extract<ExtensionMatchLiveResult, { ok: true }>
): Promise<void> {
  try {
    if (!(await getShowFitBadge())) return;
    const score = Math.round(result.combined);

    let applied: FitBadgeView['applied'] = null;
    try {
      const check = await getClient().checkApplied(url);
      if (check.found) applied = check.status === 'applied' ? 'applied' : 'saved';
    } catch {
      // Best-effort — the badge still renders without the saved/applied chip.
    }

    const view: FitBadgeView = {
      score,
      band: fitBadgeScoreBand(score),
      scoreLabel: FIT_BADGE_SCORE_SOURCE_LABEL[result.scoreSource],
      gaps: result.gaps,
      applied,
    };
    if (result.salary) view.salary = result.salary;

    await injectFitBadge(tabId, url, view);
  } catch {
    // Never let a badge-rendering failure surface anywhere — see this
    // function's own doc.
  }
}

/**
 * A tab the extension can NEVER read, so "reload the job page" would be a lie.
 * SHARED by the match-live ("Check fit") and stamp-results ("Stamp this
 * results page") gestures: the wording is deliberately gesture-neutral —
 * "there's nothing to work with here" rather than "nothing to score", which
 * would be wrong copy to answer a Stamp-results tap with (#1219).
 * Chrome redacts the url of restricted tabs (chrome://, about:*, the built-in
 * PDF viewer, the Web Store) to an EMPTY string unless the extension holds
 * `tabs` permission — which it deliberately does not (least-privilege; it
 * relies on `activeTab` instead). So both an empty url AND a readable-but-
 * restricted one fold into this message. Kinds:
 *   - browser-internal schemes: `chrome://`, `about:*`, `chrome-extension://`,
 *     `moz-extension://`
 *   - the built-in PDF viewer (`resource://pdf.js/`)
 *   - the Chrome Web Store host (a storefront, never a posting)
 *   - any pathname ending `.pdf` (a file download, never a live page)
 * Lowercased so scheme/host matching is case-insensitive without a `URL` parse
 * (some restricted urls — `about:blank` — parse fine, but a raw prefix check
 * keeps this robust for every scheme the browser hands out).
 */
const UNREADABLE_PAGE_MSG =
  "This page can't be read by the extension — there's nothing to work with here.";
/** Transient capture failure on a NORMAL url — the reload hint is truthful here.
 *  Shared by both gestures (match-live and stamp-results, #1219). */
const CAPTURE_FAILED_MSG = 'Could not read this page. Reload it and try again.';

/**
 * Is `url` a permanently-unreadable page kind (see {@link UNREADABLE_PAGE_MSG})?
 * Pure and side-effect-free so the popup copy that answers a match-live or
 * stamp-results tap on one can be unit-tested directly.
 */
function isPermanentlyUnreadablePage(rawUrl: string): boolean {
  const trimmed = rawUrl.trim().toLowerCase();
  if (
    trimmed.startsWith('chrome://') ||
    trimmed.startsWith('about:') ||
    trimmed.startsWith('chrome-extension://') ||
    trimmed.startsWith('moz-extension://') ||
    trimmed.startsWith('resource://pdf.js/')
  ) {
    return true;
  }
  try {
    const parsed = new URL(trimmed);
    if (parsed.hostname === 'chromewebstore.google.com') return true;
    if (parsed.pathname.toLowerCase().endsWith('.pdf')) return true;
  } catch {
    // Unparsable, scheme-less strings are NOT restricted here — the caller's
    // `url === ''` check and `captureTabHtml`'s own failure cover those.
  }
  return false;
}

/**
 * User-clicked "Check fit". Mirrors `runAnswersSuggest`'s not-paired
 * short-circuit (token checked BEFORE the capture injection) and its
 * never-fold-errors discipline — a deliberate click, so failures propagate to
 * `handleRequest`'s outer catch. UNLIKE `runImport`, there is no URL-only
 * fallback: `match.live` requires the captured DOM (no URL-mode network fetch
 * on the desktop side — see `extension_bridge::match_live`'s doc), so a
 * capture failure (restricted page, scripting permission denied) surfaces as
 * a user-facing error instead of silently degrading. Restricted pages are
 * caught EARLY by {@link isPermanentlyUnreadablePage} (querying the tab
 * itself, so the redacted empty url Chrome hands us for chrome://, about:* and
 * the PDF viewer folds in) and answered with {@link
 * UNREADABLE_PAGE_MSG} — the transient {@link
 * CAPTURE_FAILED_MSG} reload hint is reserved for capture failures
 * on pages that genuinely CAN be read, where "reload" is truthful (#1219).
 */
async function runMatchLive(windowId?: number): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  // Resolve the tab identity ONCE — the url, the html capture, and (after the
  // round trip below) the badge injection must all target the SAME tab, not
  // "whichever tab happens to be active" at each of three separate points in
  // time (PR review finding: a tab switch mid-request could otherwise paint
  // one page's score onto a different page).
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  const url = tab?.url ?? '';
  // A restricted tab arrives with `tab.url` redacted to '' (no `tabs`
  // permission — the url is only visible while `activeTab` is granted), or as
  // a readable clickable-restricted kind. Both mean there is nothing to score:
  // answer truthfully instead of the transient reload hint, which is a lie on
  // a page that can never be read.
  if (typeof tabId !== 'number' || url === '' || isPermanentlyUnreadablePage(url)) {
    return { ok: false, error: UNREADABLE_PAGE_MSG };
  }
  let html: string;
  try {
    html = await captureTabHtml(tabId);
  } catch {
    return { ok: false, error: CAPTURE_FAILED_MSG };
  }

  const payload: ExtensionMatchLiveRequest = { url, html };
  const result = await getClient().matchLive(payload);
  if (result.ok) {
    // Fire-and-forget: the badge is a UI enhancement on top of an already-
    // resolved Check-fit, never something the popup's own response waits on.
    // Re-verify the SAME tab still has the SAME url right before injecting —
    // a tab switch or same-tab navigation during the (possibly slow) desktop
    // round trip must abort the badge silently rather than mis-paint it.
    void (async () => {
      try {
        if (!(await tabStillOnExactUrl(tabId, url, windowId))) return;
        await maybeShowFitBadge(tabId, url, result);
      } catch {
        // No active tab to render into — skip silently.
      }
    })();
  }
  return { ok: true, kind: 'matchLive', result };
}

/**
 * User-clicked "Help me answer…" (draft, `mode` omitted) — the first
 * BILLABLE-AI verb on the bridge — OR (PR 11) a rewrite preset/submit
 * (`mode: 'rewrite'`, `existingAnswer`/`preset`/`instruction`). Both ride the
 * SAME opt-in, streaming path, and single-flight buffer below; only the
 * payload fields forwarded to the desktop differ. Mirrors `runMatchLive`'s
 * not-paired short-circuit and never-fold-errors discipline — a deliberate
 * click, so failures propagate to `handleRequest`'s outer catch. Sends the
 * active tab's url too (when readable) so the desktop can resolve grounding
 * context from a matched Application (draft mode only — rewrite mode never
 * uses it); a url-read failure degrades to generic grounding rather than
 * blocking the request (unlike `runMatchLive`, this verb has no DOM
 * dependency of its own).
 *
 * The desktop now STREAMS the answer: this resets `assistBuffer` and
 * accumulates each `assist.chunk` delta into it (broadcasting a live push
 * per chunk, see `broadcastAssistProgress`), so a popup that closes
 * mid-stream and reopens can reattach via `{kind:'answerAssistProgress'}`.
 * On any settle (success, a resolved `ok:false`, or a transport rejection)
 * the buffer is marked `done`; `interrupted` is set only when text had
 * already accumulated before the failure (a clean upfront refusal is not an
 * interruption — `resolveAnswerAssistResponse`'s `result.error` already
 * covers that case).
 *
 * Single-flight via {@link assistGeneration}: a popup closing mid-stream and
 * reopening can re-trigger this while the first run is still in flight (its
 * button isn't re-disabled on reattach). The `gen` captured on entry
 * supersedes any prior run. Two separate guards cover the two windows a
 * superseded run could otherwise clobber {@link assistBuffer} in:
 *   - BEFORE the reset (this function's own `getToken`/`activeTabUrl` awaits
 *     can still be pending after a newer call has already reset AND finished
 *     the buffer) — the early-bail right after those awaits and before the
 *     reset means a superseded run never resets the buffer a newer run
 *     already owns, and never issues its own (billable) streaming request.
 *   - DURING the stream — each chunk AND the terminal write on both the
 *     success and the error path re-check `gen` still matches
 *     {@link assistGeneration} and are a no-op when it doesn't
 *     (result/rethrow still happen normally so this run's own caller settles
 *     correctly).
 * Together these mean a superseded run can never clobber the buffer a newer
 * run owns, at any point in its lifetime.
 */
async function runAnswerAssist(
  question: string,
  searchWeb: boolean,
  mode?: 'draft' | 'rewrite',
  existingAnswer?: string,
  preset?: ExtensionRewritePreset,
  instruction?: string,
  rowId?: string,
  maxChars?: number,
  topic?: ExtensionAnswerAssistRequest['topic'],
  windowId?: number
): Promise<PopupResponse> {
  const gen = ++assistGeneration;
  const streamKind: 'draft' | 'rewrite' = mode === 'rewrite' ? 'rewrite' : 'draft';

  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  let url: string | undefined;
  try {
    url = await activeTabUrl(windowId);
  } catch {
    url = undefined;
  }
  const tabIdForRun = await activeTabId(windowId).catch(() => null);

  // A newer overlapping call already reset (and may have already finished)
  // the buffer while the awaits above were pending — this run must not reset
  // it back to `done:false`, must not broadcast, and must not make its own
  // (billable) streaming request. No await separates this check from the
  // reset below, so no third run can interleave between them.
  if (gen !== assistGeneration) {
    return { ok: false, error: 'Superseded by a newer request.' };
  }

  assistTabId = tabIdForRun;
  assistBuffer = {
    text: '',
    done: false,
    interrupted: false,
    rowId: rowId ?? '',
    kind: streamKind,
    topic: topic ?? null,
  };
  void broadcastAssistProgress();

  const payload: ExtensionAnswerAssistRequest = { question, searchWeb };
  if (url) payload.url = url;
  if (mode) payload.mode = mode;
  if (existingAnswer !== undefined) payload.existingAnswer = existingAnswer;
  if (preset) payload.preset = preset;
  if (instruction) payload.instruction = instruction;
  if (topic) payload.topic = topic;
  // DRAFT MODE ONLY (ADR-044 decision 6): the wire ignores the limit in
  // rewrite mode, so sending it there would be a claim the desktop does not
  // honour. The value is page-derived — clamp it to the shared bound before it
  // leaves, even though the desktop clamps it again.
  const limit = clampMaxChars(maxChars);
  if (streamKind === 'draft' && limit !== undefined) payload.maxChars = limit;
  try {
    const result = await getClient().answerAssist(payload, (delta) => {
      if (gen !== assistGeneration) return; // superseded — drop this late chunk
      assistBuffer = {
        ...assistBuffer,
        text: growAssistDraft(assistBuffer.text, delta),
        done: false,
        interrupted: false,
      };
      void broadcastAssistProgress();
    });
    if (gen === assistGeneration) {
      assistBuffer = {
        ...assistBuffer,
        text: result.ok ? result.draft : assistBuffer.text,
        done: true,
        interrupted: !result.ok && assistBuffer.text.length > 0,
      };
      void broadcastAssistProgress();
      // A finished run becomes a VERSION on its row (session-only, ADR-033
      // untouched). A refusal becomes the row's error instead, verbatim, so
      // the panel can match it against the shared refusal sentinels.
      if (rowId) await settleRowFromAssist(rowId, streamKind, result);
    }
    return { ok: true, kind: 'answerAssist', result };
  } catch (err) {
    if (gen === assistGeneration) {
      assistBuffer = {
        ...assistBuffer,
        done: true,
        interrupted: assistBuffer.text.length > 0,
      };
      void broadcastAssistProgress();
    }
    throw err;
  }
}

/** Bound a page-derived character limit to the shared wire ceiling. Rejects
 *  anything that is not a positive integer — a page that writes
 *  `maxlength="abc"` or a negative value must contribute nothing, not a
 *  nonsense limit the desktop then has to argue with. */
function clampMaxChars(value: number | undefined): number | undefined {
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  const floored = Math.floor(value);
  if (floored <= 0) return undefined;
  return Math.min(floored, EXTENSION_ANSWER_ASSIST_MAX_CHARS);
}

/** The neutral notice for a chip rewrite that came back unchanged (measured
 *  live, same defect class as the desktop's F3 — see `isUnchangedRewrite`). */
const UNCHANGED_REWRITE_NOTICE =
  'That came back the same — try Regenerate for a fresh draft, or a different instruction.';

/**
 * Fold a settled `answer.assist` reply into its row: a success appends the
 * new version (and selects it), a refusal records the error text verbatim so
 * the view can match the shared sentinels. Never throws — the caller's own
 * response is what settles the click.
 *
 * A REWRITE (never a draft — Regenerate is expected to differ, a chip is not)
 * whose result is unchanged from the version it reshaped is not appended as a
 * fresh version at all: that would present a no-op as if it worked, with
 * Accept enabled on text identical to what is already on screen. It becomes a
 * neutral per-row {@link AnswerRow.notice} instead.
 */
async function settleRowFromAssist(
  rowId: string,
  kind: 'draft' | 'rewrite',
  result:
    | { ok: true; draft: string; sourced: Record<string, boolean | undefined> }
    | { ok: false; error: string }
): Promise<void> {
  if (assistTabId === null) return;
  await updateAnswerState(assistTabId, (state) => {
    if (!result.ok) {
      return {
        ...state,
        rows: state.rows.map((row) => {
          if (row.id !== rowId) return row;
          const next: AnswerRow = { ...row, error: result.error };
          delete next.notice;
          return next;
        }),
      };
    }
    if (kind === 'rewrite') {
      const row = state.rows.find((r) => r.id === rowId);
      if (row && isUnchangedRewrite(rewriteBaseText(row), result.draft)) {
        return {
          ...state,
          rows: state.rows.map((r) => {
            if (r.id !== rowId) return r;
            const next: AnswerRow = { ...r, notice: UNCHANGED_REWRITE_NOTICE };
            delete next.error;
            return next;
          }),
        };
      }
    }
    // A rewrite is grounded on nothing, so it carries no flags at all rather
    // than three falses that would render an empty "grounded on" line.
    const sourced =
      kind === 'draft'
        ? {
            web: result.sourced.web === true,
            brief: result.sourced.brief === true,
            salary: result.sourced.salary === true,
          }
        : undefined;
    return { ...state, rows: appendVersion(state.rows, rowId, result.draft, kind, sourced) };
  });
}

/**
 * Inject the single-field filler into the active tab and run it against
 * `(question, index)` — refusing unless the CURRENT count of same-question
 * fields still equals scan-time `count` — with `answer`. Two-step like
 * `injectFill`: the answer text (the user's own past answer) is passed in
 * transiently via the second `executeScript({ func, args })` rather than
 * baked into the `files` injection.
 */
async function injectAnswerFill(
  question: string,
  index: number,
  count: number,
  answer: string,
  windowId?: number
): Promise<FillAnswerResult> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to fill.');

  await browser.scripting.executeScript({ target: { tabId }, files: ['answer-fill.js'] });

  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (q: string, i: number, c: number, a: string, key: string): FillAnswerResult | null => {
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((q: string, i: number, c: number, a: string) => FillAnswerResult) | undefined;
      return runner ? runner(q, i, c, a) : null;
    },
    args: [question, index, count, answer, ANSWER_FILL_GLOBAL],
  });

  const result = results[0]?.result;
  if (!isFillAnswerResult(result)) {
    throw new Error('Could not fill this field.');
  }
  return result;
}

/**
 * Per-row "Fill this field" click. Like `runStatusUpdate`, failures are NOT
 * folded away (a deliberate click) and this NEVER fills a different field
 * than the one that was scanned — `injectAnswerFill`/`fillAnswerField` fail
 * safe (`{filled:false, error}`) on any page mutation since the scan,
 * including a same-labelled field inserted elsewhere since then (`count`
 * mismatch).
 */
async function runAnswerFill(
  question: string,
  index: number,
  count: number,
  answer: string,
  windowId?: number
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const result = await injectAnswerFill(question, index, count, answer, windowId);
  return { ok: true, kind: 'answerFill', result };
}

/**
 * Inject the single-field REPLACER into the active tab and run it against
 * `(question, index)` — refusing unless the CURRENT count of same-question
 * FILLED fields still equals pick-time `count`, AND unless the field's
 * CURRENT text still equals `expectedValue` (never overwrite a manual edit
 * made since the pick — see `replaceFilledField`'s doc) — with `text`.
 * Two-step like `injectAnswerFill`: the replacement text (the AI-rewritten
 * draft, or the frozen original answer on Restore) is passed in transiently
 * via the second `executeScript({ func, args })` rather than baked into the
 * `files` injection.
 */
async function injectAnswerReplace(
  question: string,
  index: number,
  count: number,
  text: string,
  expectedValue: string,
  windowId?: number
): Promise<FillAnswerResult> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to fill.');

  await browser.scripting.executeScript({ target: { tabId }, files: ['answer-replace.js'] });

  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (
      q: string,
      i: number,
      c: number,
      t: string,
      ev: string,
      key: string
    ): FillAnswerResult | null => {
      const runner = (globalThis as Record<string, unknown>)[key] as
        ((q: string, i: number, c: number, t: string, ev: string) => FillAnswerResult) | undefined;
      return runner ? runner(q, i, c, t, ev) : null;
    },
    args: [question, index, count, text, expectedValue, ANSWER_REPLACE_GLOBAL],
  });

  const result = results[0]?.result;
  if (!isFillAnswerResult(result)) {
    throw new Error('Could not replace this field.');
  }
  return result;
}

/**
 * Rewrite mode's Accept/Restore click (PR 11) — SAME request kind, only
 * `text` differs. Like `runAnswerFill`, failures are NOT folded away and
 * this NEVER replaces a different field than the one that was picked, NOR a
 * field whose CURRENT text no longer matches `expectedValue` (a manual edit
 * since the pick) — `injectAnswerReplace`/`replaceFilledField` fail safe
 * (`{filled:false, error}`) on either. Never submits the form.
 */
async function runAnswerReplace(
  question: string,
  index: number,
  count: number,
  text: string,
  expectedValue: string,
  windowId?: number
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const result = await injectAnswerReplace(question, index, count, text, expectedValue, windowId);
  return { ok: true, kind: 'answerReplace', result };
}

// ── shared answer state (ADR-044) ────────────────────────────────────────────

/** The active tab's id. Available WITHOUT the `tabs` permission (only a tab's
 *  url/title are gated behind it), which is what lets the state be keyed per
 *  tab while `tabs` stays on the manifest denylist. */
async function activeTabId(windowId?: number): Promise<number> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab.');
  return tabId;
}

/**
 * The active tab's ORIGIN, read at GESTURE TIME (ADR-044 decision 1 and the
 * design log's amendment 10d). This only works because the click that got us
 * here just granted `activeTab` for this tab, which is what makes its url
 * readable — it is NOT a `tabs`-permission lookup, and `tabs` stays on the
 * denylist. Degrades to `''` rather than throwing: an unreadable origin costs
 * the state its "same page?" check, never the scan.
 */
async function activeTabOriginAtGesture(windowId?: number): Promise<string> {
  try {
    return new URL(await activeTabUrl(windowId)).origin;
  } catch {
    return '';
  }
}

/** Minimal guard for `capture-rows.js`'s completion value across the
 *  `executeScript` boundary — same discipline as `isScannedQuestions`. */
function isAnswerScan(v: unknown): v is AnswerScan {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return isScannedQuestions(o.questions) && isFilledFields(o.filled);
}

/** Inject the answer-rows collector into the active tab and return its scan. */
async function captureActiveTabRows(tabId: number): Promise<AnswerScan> {
  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['capture-rows.js'],
  });
  const scan = results[0]?.result;
  if (!isAnswerScan(scan)) throw new Error('Could not read the questions on this page.');
  return scan;
}

/**
 * Which of `questions` a past application can already answer, and with what.
 * BEST-EFFORT: every failure (not paired, bridge down, the autofill opt-in
 * off, a desktop refusal) folds to an empty map, so a row simply stays
 * `empty` instead of the whole scan failing over a status badge. Salary-shaped
 * suggestions are dropped for the same reason the suggestion rows never offer
 * to fill them.
 */
async function savedAnswersFor(
  questions: string[]
): Promise<Map<string, { answer: string; source?: string }>> {
  const out = new Map<string, { answer: string; source?: string }>();
  if (questions.length === 0) return out;
  try {
    const result = await getClient().suggestAnswers(questions.slice(0, MAX_SUGGEST_QUESTIONS));
    if (!result.ok) return out;
    for (const s of result.suggestions) {
      if (s.salary || out.has(s.question)) continue;
      const source = [s.sourceTitle, s.sourceCompany].filter(Boolean).join(' at ');
      out.set(s.question, source ? { answer: s.answer, source } : { answer: s.answer });
    }
  } catch {
    // Folded on purpose — see this function's doc.
  }
  return out;
}

/**
 * The gesture that (re)builds the shared answer state for the active tab:
 * inject the rows collector, capture the origin, look up saved answers, and
 * write the result to `storage.session`, where BOTH surfaces are subscribed.
 *
 * Versions already drafted on a row survive the rescan (`buildRows`), so
 * running this on every popup open — and on the panel's Rescan for a
 * multi-step form — never costs the user work.
 */
async function runAnswerScan(windowId?: number): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const origin = await activeTabOriginAtGesture(windowId);
  const scan = await captureActiveTabRows(tabId);
  const previous = await readAnswerState(tabId);
  const savedFor = await savedAnswersFor([...new Set(scan.questions.map((q) => q.question))]);

  const state: AnswerState = {
    tabId,
    origin,
    scannedAt: Date.now(),
    rows: buildRows(scan, savedFor, previous?.rows ?? []),
    stream: previous?.stream ?? null,
    // The scan itself IS the re-arm: it only ran because a gesture granted
    // `activeTab` for this tab, so whatever navigation set the flag is now
    // accounted for.
    pageChanged: false,
  };
  await writeAnswerState(state);
  return { ok: true, kind: 'answerState', state };
}

/** Add (or reuse) a free-text row — the manual entry and the context-menu
 *  selection both land here. No page access: a question the scan missed is
 *  still a question worth drafting, it just has nowhere to be accepted into.
 *  `explicitTabId` lets a caller that already has the right tab (the
 *  context-menu gesture) pin the row to it directly — falls back to the
 *  active-tab query only when absent, so a focus change between the gesture
 *  and this call can't land the row in the wrong tab's record. */
async function runAnswerAddRow(
  question: string,
  explicitTabId?: number,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = explicitTabId ?? (await activeTabId(windowId));
  const existing = await readAnswerState(tabId);
  const state: AnswerState = existing ?? {
    tabId,
    origin: await activeTabOriginAtGesture(windowId),
    scannedAt: Date.now(),
    rows: [],
    stream: null,
    // Nothing has been scanned, so nothing claims to know the page — the
    // write controls are gated on a row HAVING a field, not on this flag.
    pageChanged: false,
  };
  const next: AnswerState = { ...state, rows: addFreeRow(state.rows, question) };
  await writeAnswerState(next);
  return { ok: true, kind: 'answerState', state: next };
}

/** Show a different version of a row. Pure state — Restore in decision 5's
 *  sense; it writes nothing to the page until the user presses Accept. */
async function runAnswerSelectVersion(
  rowId: string,
  version: number,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const state = await updateAnswerState(tabId, (current) => ({
    ...current,
    rows: current.rows.map((row) =>
      row.id === rowId
        ? { ...row, selected: version >= 0 && version < row.versions.length ? version : -1 }
        : row
    ),
  }));
  return { ok: true, kind: 'answerState', state };
}

/** Find a row by id, or throw the message the surface renders. */
function requireRow(state: AnswerState | null, rowId: string): AnswerRow {
  const row = state?.rows.find((r) => r.id === rowId);
  if (!row) throw new Error('That question is no longer on this page — rescan and try again.');
  return row;
}

/**
 * Write `text` into `row`'s field through the SAME fail-safe path the popup's
 * per-row Fill and rewrite Accept already use, chosen by the row's field kind:
 * an `empty` field goes through `answer-fill.js` (which refuses unless the
 * same-question EMPTY field count still matches), a `filled` one through
 * `answer-replace.js` (which refuses that AND any text that is no longer what
 * we believe is in the field). On success the row's `currentText` moves to
 * what was written, so a second Accept still knows what it is replacing.
 */
async function writeRowText(
  rowId: string,
  text: string,
  windowId?: number
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const tabId = await activeTabId(windowId);
  const state = await readAnswerState(tabId);
  if (state?.pageChanged) {
    return {
      ok: false,
      error: 'This page changed. Click the toolbar icon to scan it, then try again.',
    };
  }
  const row = requireRow(state, rowId);
  const field = row.field;
  if (!field) {
    return { ok: false, error: 'This question is not on the page, so there is nothing to fill.' };
  }

  const result =
    field.kind === 'empty'
      ? await injectAnswerFill(row.question, field.index, field.count, text, windowId)
      : await injectAnswerReplace(
          row.question,
          field.index,
          field.count,
          text,
          field.currentText,
          windowId
        );

  if (result.filled) {
    await updateAnswerState(tabId, (current) => ({
      ...current,
      rows: current.rows.map((r) =>
        r.id === rowId && r.field ? { ...r, field: { ...r.field, currentText: text } } : r
      ),
    }));
  }
  return { ok: true, kind: 'answerAccept', result };
}

/** Accept: write the version currently on screen into the field. */
async function runAnswerAccept(rowId: string, windowId?: number): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const row = requireRow(await readAnswerState(tabId), rowId);
  return writeRowText(rowId, selectedText(row), windowId);
}

/** Restore original: put the field's FROZEN scan-time text back. */
async function runAnswerRestoreOriginal(rowId: string, windowId?: number): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const row = requireRow(await readAnswerState(tabId), rowId);
  return writeRowText(rowId, row.field?.originalText ?? '', windowId);
}

/**
 * Draft or rewrite for one row, resolved against the row's OWN state so the
 * two verbs can never be confused at the call site: a rewrite starts from the
 * row's latest version (never the selected one — see `rewriteBaseText`) and
 * carries no limit; a draft carries the field's `maxlength` and no existing
 * answer. This is the single place ADR-044 decision 5's "chips reshape,
 * Regenerate rethinks" becomes two different requests.
 */
async function runAnswerRowAssist(
  rowId: string,
  searchWeb: boolean,
  mode: 'draft' | 'rewrite',
  preset?: ExtensionRewritePreset,
  instruction?: string,
  windowId?: number
): Promise<PopupResponse> {
  const tabId = await activeTabId(windowId);
  const row = requireRow(await readAnswerState(tabId), rowId);
  if (mode === 'rewrite') {
    const base = rewriteBaseText(row);
    if (!base.trim()) {
      return { ok: false, error: 'There is nothing to reshape yet — draft an answer first.' };
    }
    return runAnswerAssist(
      row.question,
      false,
      'rewrite',
      base,
      preset,
      instruction,
      rowId,
      undefined,
      undefined,
      windowId
    );
  }
  return runAnswerAssist(
    row.question,
    searchWeb,
    'draft',
    undefined,
    undefined,
    instruction,
    rowId,
    row.field?.maxChars,
    undefined,
    windowId
  );
}

/** Central popup-request dispatcher. Never throws — maps errors to `ok:false`. */
async function dispatchRequest(req: PopupRequest): Promise<PopupResponse> {
  try {
    switch (req.kind) {
      case 'getStatus': {
        // Opening the popup is a good moment to (re)probe the bridge, and to
        // clear any pending auto-track "import this untracked job?" nudge (the
        // user is now here and can act on it via the Import button).
        void getClient().ensureConnected();
        clearImportPrompt();
        const status = await computeStatus();
        return { ok: true, kind: 'status', status };
      }
      case 'setToken': {
        if (!looksLikeToken(req.token)) {
          return {
            ok: false,
            error:
              'Invalid token format. Paste the full 64-character hex token from the desktop app.',
          };
        }
        await setToken(req.token);
        // Reset any bad-token block so the bridge will attempt auth with the new token.
        getClient().resetForNewToken();
        void getClient().ensureConnected();
        return { ok: true, kind: 'token' };
      }
      case 'clearToken': {
        // Same local un-pair the desktop's `token.revoked` triggers — clears
        // the stored token and any bad-token block (bridge → searching).
        await unpairLocally();
        return { ok: true, kind: 'token' };
      }
      case 'reconnect': {
        await getClient().ensureConnected();
        return { ok: true, kind: 'status', status: await computeStatus() };
      }
      case 'import':
        return await runImport(req.applied, req.windowId);
      case 'fill':
        return await runFill(req.windowId);
      case 'profileGet':
        return await runProfileGet();
      case 'appliedCheck':
        return await runAppliedCheck(req.windowId);
      case 'fieldsProbe':
        return await runFieldsProbe(req.windowId);
      case 'autofillCheck':
        return await runAutofillCheck();
      case 'trustLineJob':
        return await runTrustLineJob(req.windowId);
      case 'settingsGet':
        return await runSettingsGet();
      case 'settingsSet':
        return await runSettingsSet(req.key, req.enabled);
      case 'statusUpdate':
        return await runStatusUpdate(req.windowId);
      case 'answersSave':
        return await runAnswersSave(req.windowId);
      case 'answersSuggest':
        return await runAnswersSuggest(req.windowId);
      case 'answerFill':
        return await runAnswerFill(req.question, req.index, req.count, req.answer, req.windowId);
      case 'matchLive':
        return await runMatchLive(req.windowId);
      case 'answerAssist':
        // A request that names a ROW is resolved against that row's own state
        // (its latest version, its field's limit) rather than trusting the
        // caller to have assembled them — see `runAnswerRowAssist`.
        return req.rowId
          ? await runAnswerRowAssist(
              req.rowId,
              req.searchWeb,
              req.mode === 'rewrite' ? 'rewrite' : 'draft',
              req.preset,
              req.instruction,
              req.windowId
            )
          : await runAnswerAssist(
              req.question,
              req.searchWeb,
              req.mode,
              req.existingAnswer,
              req.preset,
              req.instruction,
              undefined,
              req.maxChars,
              req.topic,
              req.windowId
            );
      case 'answerAssistProgress':
        return {
          ok: true,
          kind: 'answerAssistProgress',
          text: assistBuffer.text,
          done: assistBuffer.done,
          interrupted: assistBuffer.interrupted,
          rowId: assistBuffer.rowId,
        };
      case 'answerScan':
        return await runAnswerScan(req.windowId);
      case 'answerAddRow':
        return await runAnswerAddRow(req.question, undefined, req.windowId);
      case 'answerSelectVersion':
        return await runAnswerSelectVersion(req.rowId, req.version, req.windowId);
      case 'answerAccept':
        return await runAnswerAccept(req.rowId, req.windowId);
      case 'answerRestoreOriginal':
        return await runAnswerRestoreOriginal(req.rowId, req.windowId);
      case 'answerReplace':
        return await runAnswerReplace(
          req.question,
          req.index,
          req.count,
          req.text,
          req.expectedValue,
          req.windowId
        );
      case 'documentsList':
        return await runDocumentsList(req.windowId);
      case 'documentExportText':
        return await runDocumentExportText(req.source, req.templateId, req.letterLayoutId);
      case 'documentAttach':
        return await runDocumentAttach(req.source, req.templateId, req.format, req.windowId);
      case 'stampResults':
        return await runStampResults(req.windowId);
      case 'prepGet':
        return await runPrepGet(req.windowId);
      case 'assistCancel':
        return runAssistCancel();
      case 'autoSaveNotice':
        return await runAutoSaveNotice();
      default: {
        // Exhaustiveness guard — a new PopupRequest variant must be handled.
        const _never: never = req;
        return { ok: false, error: `Unknown request: ${JSON.stringify(_never)}` };
      }
    }
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}

/**
 * Popup-request entry: dispatch, then — after a SUCCESSFUL page-touching
 * gesture — arm the auto-track submit watcher on that page (opt-in gated +
 * idempotent per page), so a subsequent form submit can auto-mark the matched
 * application applied. Arming is fire-and-forget: it never affects the popup's
 * own response.
 */
async function handleRequest(req: PopupRequest): Promise<PopupResponse> {
  const response = await dispatchRequest(req);
  if (response.ok && GESTURE_KINDS.has(req.kind)) {
    void maybeArmSubmitWatch({
      autotrackEnabled: () => getClient().autotrackEnabled(),
      // Bind the requesting surface's window id into the dep so the ARM (a
      // fire-and-forget step that runs after this request settles) targets
      // the tab the gesture actually happened on, not whichever window the
      // browser focused last (#1215) — `maybeArmSubmitWatch`'s own signature
      // stays untouched: every request is concurrent in a service worker, so
      // the window has to ride the dep closure, never module state.
      injectSubmitWatch: (captureAnswers) => injectSubmitWatch(captureAnswers, req.windowId),
      saveAnswersOnSubmitEnabled,
    });
  }
  return response;
}

// ── wiring ────────────────────────────────────────────────────────────────────

browser.runtime.onMessage.addListener(
  (
    message: unknown,
    sender: Browser.runtime.MessageSender,
    sendResponse: (response?: PopupResponse) => void
  ): true | undefined => {
    // The injected submit-watcher posts a fire-and-forget `submitDetected` — it
    // is NOT a popup request and expects no response, so handle it out-of-band.
    if (isSubmitDetected(message)) {
      // Belt-and-braces MV3 hygiene: this extension declares no
      // `externally_connectable`, so no other extension/page can ever reach
      // this listener — but require the sender to be THIS extension anyway
      // before acting on it (defense-in-depth, costs nothing).
      if (sender.id === browser.runtime.id) {
        void handleSubmitDetected(message.url, submitFlowDeps(), message.answers);
      }
      return undefined;
    }
    // The injected fit badge's "Open the panel" button — also fire-and-forget,
    // same sender-check discipline as `isSubmitDetected` above.
    if (isOpenPanelFromBadge(message)) {
      if (sender.id === browser.runtime.id && typeof sender.tab?.id === 'number') {
        openAnswerPanel(sender.tab.id);
      }
      return undefined;
    }
    // Reply via `sendResponse` + a LITERAL `true`, never by returning a Promise.
    // `@wxt-dev/browser` is a thin `browser ?? chrome` pass-through (its README
    // says so explicitly — it is not `webextension-polyfill`), and Chromium's
    // `chrome.runtime.onMessage` has never supported a Promise return value: it
    // keeps the channel open only for a literal `true`. A returned Promise is
    // truthy but not `true`, so the channel closed immediately, `sendMessage`
    // resolved `undefined`, and every request/response action (import, fill,
    // save, mark-applied, check-fit, status) came back as "No response from the
    // extension background." — while the one-way background→popup pushes kept
    // working, so the pill could still read "Connected".
    //
    // `sendResponse` + `return true` is the shape BOTH engines accept, so this
    // is correct on Firefox regardless.
    void handleRequest(message as PopupRequest).then(sendResponse, (err: unknown) => {
      // A rejection here would otherwise leave the port open until it times out,
      // which the popup surfaces as the same "No response" message.
      sendResponse({ ok: false, error: err instanceof Error ? err.message : String(err) });
    });
    return true;
  }
);

/**
 * The selection-scoped context-menu entry (ADR-044 decision 2, amended — see
 * `ANSWER_PANEL_MENU_ID` for the second entry). It is registered on
 * `selection` only, so it never appears on a page the user has not selected
 * text on, and clicking it is ITSELF the gesture that grants `activeTab` for
 * that tab — one of the gestures Chrome documents for `sidePanel.open`.
 */
const ANSWER_MENU_ID = 'ajh-answer-selection';

/**
 * The plain-right-click entry: opens the panel with nothing prefilled, so it
 * has no selection to require. Registered on `contexts: ['page', 'editable']`
 * rather than `'all'`, which would also fire on links/images/video — more
 * than a bare "open the panel" gesture needs.
 *
 * `'page'` is Chrome's LEAST-specific context, not an independent one: per
 * Chromium's own matching rule (`ExtensionContextAndPatternMatch`,
 * `chrome/browser/extensions/context_menu_helpers.cc`), a `page`-scoped item
 * is suppressed whenever a selection, a link, an editable field, or a media
 * element is under the cursor — those all take priority over the fallback.
 * So the two entries are mutually exclusive in the common case, not stacked:
 *   - plain background, nothing selected, not editable: only this entry.
 *   - selected text, not editable: only `ANSWER_MENU_ID` (`page` suppressed
 *     by the selection).
 *   - inside an editable field, nothing selected: only this entry
 *     (`editable` is one of its two declared contexts — added because the
 *     primary use of this extension is answering questions inside
 *     application-form fields, and a bare `page` context is suppressed
 *     there too, showing nothing).
 *   - inside an editable field WITH a selection: BOTH match (this entry via
 *     `editable`, `ANSWER_MENU_ID` via `selection`) — the one case where
 *     Chrome's documented "multiple visible items collapse into a single
 *     parent submenu" behaviour
 *     (developer.chrome.com/docs/extensions/reference/api/contextMenus, read
 *     2026-09-05) actually applies, titled with this extension's own
 *     manifest `name` (`'AI Job Hunter — Job Importer'`, see `manifest.ts`),
 *     not a shorter label.
 */
const ANSWER_PANEL_MENU_ID = 'ajh-answer-open-panel';

/** Longest selection accepted as a question. A selection is untrusted page
 *  content; the desktop clamps it again, this just avoids carrying a whole
 *  article into the row list. */
const MAX_SELECTION_QUESTION = 500;

/** (Re)create the context-menu entries. `removeAll` first because
 *  `onInstalled` fires on every update and `create` throws on a duplicate id,
 *  which would otherwise poison the whole listener. */
function installContextMenu(): void {
  const menus = browser.contextMenus;
  if (!menus) return;
  menus.removeAll(() => {
    menus.create({
      id: ANSWER_MENU_ID,
      title: 'Answer this with AI Job Hunter',
      contexts: ['selection'],
    });
    menus.create({
      id: ANSWER_PANEL_MENU_ID,
      title: 'Open AI Job Hunter answer tool',
      contexts: ['page', 'editable'],
    });
  });
}

/**
 * Origin from a tab's own `url` field, captured DIRECTLY from the
 * context-menu event's `tab` object — never a fresh `browser.tabs.query`.
 * Same degrade-to-`''`-on-parse-failure contract as `activeTabOriginAtGesture`
 * (missing/malformed url → `''`, never a throw), but sourced from a value the
 * caller already has at gesture time instead of a live re-query, which could
 * resolve to a DIFFERENT tab if the user switched away during an intervening
 * `await` (see `rearmPageChangedForGesture`'s doc for why that distinction
 * matters here).
 */
function originFromTabUrl(url: string | undefined): string {
  try {
    return new URL(url ?? '').origin;
  } catch {
    return '';
  }
}

/**
 * Force `pageChanged: false` for `tabId`'s answer state ahead of a
 * context-menu gesture — a right-click IS a qualifying `activeTab` gesture
 * (per `runFieldsProbe`/`runAppliedCheck`'s doc), so both context-menu
 * handlers must re-arm the panel's job-tools trust gate exactly like
 * `runAnswerScan`'s "the scan itself IS the re-arm" comment, WITHOUT actually
 * scanning: a bare right-click implies nothing about wanting to re-scan the
 * page's Answer-tools questions. `updateAnswerState` no-ops when no record
 * exists yet, so a fresh tab still needs the SAME minimal record
 * `runAnswerAddRow` builds for one — built here too rather than via a
 * throwaway `runAnswerAddRow('', tabId)` call, which would silently do
 * nothing useful for an EXISTING record (it never clears a stale
 * `pageChanged`).
 *
 * `origin` is a PARAMETER, not re-derived internally: this function already
 * awaits `updateAnswerState` before it would need one, and deriving it via a
 * fresh `activeTabOriginAtGesture()` (which queries whichever tab is
 * CURRENTLY active) at that point could read a DIFFERENT tab's url if the
 * user switched tabs/windows in the interim — the origin has to come from
 * the gesture's own `tab.url`, captured by the caller before any await (see
 * `originFromTabUrl`).
 */
async function rearmPageChangedForGesture(tabId: number, origin: string): Promise<void> {
  const updated = await updateAnswerState(tabId, (state) => ({ ...state, pageChanged: false }));
  if (updated) return;
  await writeAnswerState({
    tabId,
    origin,
    scannedAt: Date.now(),
    rows: [],
    stream: null,
    pageChanged: false,
  });
}

/**
 * Context-menu click: add the selection as a free-text row, then open the
 * panel. `open` is called from inside this handler because THIS click is the
 * user gesture — anything awaited before it loses the gesture, which is why
 * the row is added after the panel is opened rather than before.
 */
async function handleAnswerMenuClick(
  info: Browser.contextMenus.OnClickData,
  tab: Browser.tabs.Tab | undefined
): Promise<void> {
  const question = (info.selectionText ?? '').trim().slice(0, MAX_SELECTION_QUESTION);
  if (!question) return;
  openAnswerPanel(tab?.id);
  if (typeof tab?.id === 'number') {
    await rearmPageChangedForGesture(tab.id, originFromTabUrl(tab.url));
  }
  await runAnswerAddRow(question, tab?.id);
}

/**
 * Open the answer panel for `tabId` on whichever browser we are on. Chrome's
 * `sidePanel.open` and Firefox's `sidebarAction.open` BOTH require a user
 * gesture and are therefore called synchronously from a click handler, never
 * after an await. The panel's `default_path` is declared in the manifest, so
 * there is no `setOptions` call to lose the gesture on (design log 10a).
 */
function openAnswerPanel(tabId: number | undefined): void {
  const chromePanel = (browser as { sidePanel?: { open(o: { tabId: number }): Promise<void> } })
    .sidePanel;
  if (chromePanel && typeof tabId === 'number') {
    void chromePanel.open({ tabId }).catch(() => {
      // A revoked gesture or a window that cannot host a panel — the popup's
      // own control is still there, so there is nothing to report here.
    });
    return;
  }
  const sidebar = (browser as { sidebarAction?: { open(): Promise<void> } }).sidebarAction;
  void sidebar?.open().catch(() => {
    // Same rationale as above.
  });
}

if (browser.contextMenus) {
  browser.contextMenus.onClicked.addListener((info, tab) => {
    if (info.menuItemId === ANSWER_PANEL_MENU_ID) {
      openAnswerPanel(tab?.id);
      if (typeof tab?.id === 'number') {
        void rearmPageChangedForGesture(tab.id, originFromTabUrl(tab.url));
      }
      return;
    }
    if (info.menuItemId !== ANSWER_MENU_ID) return;
    void handleAnswerMenuClick(info, tab);
  });
}

/**
 * A navigation in a tab invalidates that tab's answer state for WRITING (the
 * `activeTab` grant may be gone and the scanned fields may be gone with it),
 * but not for READING — decision 3 keeps the rows and replaces the write
 * controls. Deliberately conservative: without the `tabs` permission the url
 * is not readable here, so a same-origin navigation flips the flag too. The
 * cost is one extra toolbar click; the alternative is a write control that
 * silently does nothing.
 */
browser.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status !== 'loading') return;
  void updateAnswerState(tabId, (state) =>
    state.pageChanged ? null : { ...state, pageChanged: true }
  );
});

// The tab is gone, and so is anything its rows referred to.
browser.tabs.onRemoved.addListener((tabId) => {
  void clearAnswerState(tabId);
});

// Re-probe on the lifecycle wake points so a freshly-started worker reconnects.
browser.runtime.onStartup.addListener(() => {
  void getClient().ensureConnected();
  installContextMenu();
});
browser.runtime.onInstalled.addListener(() => {
  void getClient().ensureConnected();
  installContextMenu();
});

// Kick an initial probe when the worker first loads.
void getClient().ensureConnected();

// Apply a pending update immediately once the browser has already downloaded it,
// instead of waiting for the next natural SW restart.
// ponytail: onUpdateAvailable only fires when an update is already staged — we
// are not pulling the update, just collapsing the apply delay.
browser.runtime.onUpdateAvailable.addListener(() => {
  browser.runtime.reload();
});

// Chrome-only: nudge the browser to check for an update now so the download
// starts sooner. requestUpdateCheck is absent in Firefox, so feature-detect.
// ponytail: single startup nudge only — the browser already polls periodically.
if (typeof browser.runtime.requestUpdateCheck === 'function') {
  void browser.runtime.requestUpdateCheck().catch((err: unknown) => {
    // Non-fatal — update checks may be rate-limited or unavailable. Surface a
    // sanitized warning to the SW console (no telemetry leaves the device) so a
    // persistent updater regression stays observable instead of fully silent.
    console.warn('[ajh] update check failed:', err instanceof Error ? err.name : 'unknown');
  });
}

// Ensure this file is treated as an ES module (Chrome SW `type: module`).
export {};
