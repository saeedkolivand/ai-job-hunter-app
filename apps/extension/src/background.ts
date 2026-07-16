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
  ExtensionImportRequest,
  ExtensionMatchLiveRequest,
  ExtensionRewritePreset,
} from '@ajh/shared';

// TYPE-ONLY import from the answer-fill module — same rationale as the
// autofill.ts import below: `answer-fill.js`/`answer-replace.js` are
// classic-script injection targets, so their runtime code must be imported
// ONLY by `answer-fill.ts`/`answer-replace.ts`.
import type { FillAnswerResult } from './lib/answer-fill';
// Same type-only rationale as the autofill.ts import above — capture.js /
// capture-questions.js are ALSO classic-script injection targets (see
// vite.config.ts's `injectedEntries`), so this import must stay type-only
// (erased at build) to keep answers-capture.ts's runtime code out of the
// background's bundle.
import type { CapturedAnswer, FilledField, ScannedQuestion } from './lib/answers-capture';
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
import type { ConnectionStatus, PopupRequest, PopupResponse } from './lib/messages';
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
 *  idempotent per page). */
const GESTURE_KINDS: ReadonlySet<PopupRequest['kind']> = new Set([
  'import',
  'fill',
  'answersSave',
  'answersSuggest',
  'answerFill',
  'answerReplace',
  'matchLive',
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
      getToken
    );
  }
  return client;
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

/** Client-side mirror of the Rust `answer_assist::DRAFT_CAP` (4000 chars) —
 *  bounds how far {@link assistBuffer}'s text can grow. The desktop already
 *  clamps each `assist.chunk` live so this should never actually trip in
 *  normal operation; it exists so the interrupted/error path (which shows
 *  whatever text had already accumulated) can never render an unbounded
 *  draft even if that server-side guarantee were ever violated. */
const ASSIST_DRAFT_CAP = 4_000;

/** Append `delta` to `text`, clamped to {@link ASSIST_DRAFT_CAP}. */
function growAssistDraft(text: string, delta: string): string {
  const grown = text + delta;
  return grown.length > ASSIST_DRAFT_CAP ? grown.slice(0, ASSIST_DRAFT_CAP) : grown;
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
let assistBuffer: { text: string; done: boolean; interrupted: boolean } = {
  text: '',
  done: true,
  interrupted: false,
};

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
  try {
    const message: PopupResponse = { ok: true, kind: 'answerAssistProgress', ...assistBuffer };
    await browser.runtime.sendMessage(message);
  } catch {
    // No popup open — fine.
  }
}

/** Resolve the active tab's URL for an import. */
async function activeTabUrl(): Promise<string> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  const url = tab?.url ?? '';
  if (!url) throw new Error('Could not read the current tab URL.');
  return url;
}

/**
 * Scan mode: inject the capture script into the active tab and return its
 * `outerHTML`. Requires `scripting` + `activeTab` (granted on the click).
 */
async function captureActiveTabHtml(): Promise<string> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to scan.');

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
async function runImport(applied: boolean): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl();
  const payload: ExtensionImportRequest = { url, applied };
  // Always try to capture the authenticated DOM so the desktop can parse it
  // without re-fetching (which would hit bot-walls on LinkedIn/Indeed/Glassdoor).
  // Fall back to URL-only if executeScript is blocked (restricted pages).
  try {
    payload.html = await captureActiveTabHtml();
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
async function injectFill(profile: AutofillProfile): Promise<AutofillSummary> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
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
async function runFill(): Promise<PopupResponse> {
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
  const summary = await injectFill(fields);
  return { ok: true, kind: 'fill', summary };
}

/**
 * Fire-and-forget "have I already applied?" check for the active tab's URL —
 * a read-only, best-effort enhancement over the import view. NEVER surfaces
 * `ok:false`: any failure (not paired, bridge unreachable, an old desktop's
 * unrecognized message type, a malformed reply) folds into `{ found: false }`
 * so the popup renders nothing rather than an error.
 */
async function runAppliedCheck(): Promise<PopupResponse> {
  try {
    const url = await activeTabUrl();
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
async function captureActiveTabFieldsProbe(): Promise<FieldsProbeResult> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
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
async function runFieldsProbe(): Promise<PopupResponse> {
  try {
    const { hasFormFields, hasAnswerFields } = await captureActiveTabFieldsProbe();
    return { ok: true, kind: 'fieldsProbe', hasFormFields, hasAnswerFields };
  } catch {
    return { ok: true, kind: 'fieldsProbe', hasFormFields: true, hasAnswerFields: true };
  }
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
async function runStatusUpdate(): Promise<PopupResponse> {
  const url = await activeTabUrl();
  const result = await getClient().updateStatus(url);
  return { ok: true, kind: 'statusUpdate', result };
}

// ── Auto-track (Task #22, Layer A) ──────────────────────────────────────────────

/** Guard for the injected submit-watcher's fire-and-forget message. */
function isSubmitDetected(v: unknown): v is { kind: 'submitDetected'; url: string } {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as Record<string, unknown>;
  return o.kind === SUBMIT_DETECTED_MSG && typeof o.url === 'string';
}

/**
 * Inject the auto-track submit watcher into the active tab (single-step, like
 * `captureActiveTabHtml`: the watcher self-arms on load and needs no argument).
 * Called only after a successful gesture + only when the opt-in is on (see
 * {@link maybeArmSubmitWatch}); the watcher's own isolated-world flag makes a
 * repeat injection on the same page a no-op.
 */
async function injectSubmitWatch(): Promise<void> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  const tabId = tab?.id;
  if (typeof tabId !== 'number') return;
  await browser.scripting.executeScript({ target: { tabId }, files: ['submit-watch.js'] });
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

/** Auto-track dependencies wired to the live bridge client. */
function submitFlowDeps() {
  return {
    autotrackEnabled: () => getClient().autotrackEnabled(),
    checkApplied: (url: string) => getClient().checkApplied(url),
    updateStatusAuto: (url: string) => getClient().updateStatus(url, true),
    promptImport,
  };
}

/**
 * Inject the answers-capture collector into the active tab and return its
 * `{answers, filled}` (see `capture.ts`'s doc). Single-step injection (unlike
 * `injectFill`'s files+func two-step): the collector takes no PII input to
 * pass in transiently, it only reads the page and returns data — same
 * pattern as `captureActiveTabHtml`.
 */
async function captureActiveTabFormData(): Promise<{
  answers: CapturedAnswer[];
  filled: FilledField[];
}> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
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
async function runAnswersSave(): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl();
  const { answers, filled } = await captureActiveTabFormData();
  const result = await getClient().saveAnswers(url, answers);
  return { ok: true, kind: 'answersSave', result, filled };
}

/**
 * Inject the questions-mode collector into the active tab and return its
 * `{question, index}[]` scan-time correlation list. Single-step injection —
 * same pattern as `captureActiveTabAnswers`.
 */
async function captureActiveTabQuestions(): Promise<ScannedQuestion[]> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
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
async function runAnswersSuggest(): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const scanned = await captureActiveTabQuestions();
  // Dedup by exact text (the desktop dedups by normalized text) and cap
  // client-side — untrusted page content, never send an unbounded array.
  const questions = [...new Set(scanned.map((q) => q.question))].slice(0, MAX_SUGGEST_QUESTIONS);
  const result = await getClient().suggestAnswers(questions);
  return { ok: true, kind: 'answersSuggest', result, scanned };
}

/**
 * User-clicked "Check fit". Mirrors `runAnswersSuggest`'s not-paired
 * short-circuit (token checked BEFORE the capture injection) and its
 * never-fold-errors discipline — a deliberate click, so failures propagate to
 * `handleRequest`'s outer catch. UNLIKE `runImport`, there is no URL-only
 * fallback: `match.live` requires the captured DOM (no URL-mode network fetch
 * on the desktop side — see `extension_bridge::match_live`'s doc), so a
 * capture failure (restricted page, scripting permission denied) surfaces as
 * a user-facing error instead of silently degrading.
 */
async function runMatchLive(): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl();
  let html: string;
  try {
    html = await captureActiveTabHtml();
  } catch {
    return { ok: false, error: 'Could not read this page. Reload the job page and try again.' };
  }

  const payload: ExtensionMatchLiveRequest = { url, html };
  const result = await getClient().matchLive(payload);
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
  instruction?: string
): Promise<PopupResponse> {
  const gen = ++assistGeneration;

  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  let url: string | undefined;
  try {
    url = await activeTabUrl();
  } catch {
    url = undefined;
  }

  // A newer overlapping call already reset (and may have already finished)
  // the buffer while the awaits above were pending — this run must not reset
  // it back to `done:false`, must not broadcast, and must not make its own
  // (billable) streaming request. No await separates this check from the
  // reset below, so no third run can interleave between them.
  if (gen !== assistGeneration) {
    return { ok: false, error: 'Superseded by a newer request.' };
  }

  assistBuffer = { text: '', done: false, interrupted: false };
  void broadcastAssistProgress();

  const payload: ExtensionAnswerAssistRequest = { question, searchWeb };
  if (url) payload.url = url;
  if (mode) payload.mode = mode;
  if (existingAnswer !== undefined) payload.existingAnswer = existingAnswer;
  if (preset) payload.preset = preset;
  if (instruction) payload.instruction = instruction;
  try {
    const result = await getClient().answerAssist(payload, (delta) => {
      if (gen !== assistGeneration) return; // superseded — drop this late chunk
      assistBuffer = {
        text: growAssistDraft(assistBuffer.text, delta),
        done: false,
        interrupted: false,
      };
      void broadcastAssistProgress();
    });
    if (gen === assistGeneration) {
      assistBuffer = {
        text: result.ok ? result.draft : assistBuffer.text,
        done: true,
        interrupted: !result.ok && assistBuffer.text.length > 0,
      };
      void broadcastAssistProgress();
    }
    return { ok: true, kind: 'answerAssist', result };
  } catch (err) {
    if (gen === assistGeneration) {
      assistBuffer = {
        text: assistBuffer.text,
        done: true,
        interrupted: assistBuffer.text.length > 0,
      };
      void broadcastAssistProgress();
    }
    throw err;
  }
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
  answer: string
): Promise<FillAnswerResult> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
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
  answer: string
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const result = await injectAnswerFill(question, index, count, answer);
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
  expectedValue: string
): Promise<FillAnswerResult> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
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
  expectedValue: string
): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const result = await injectAnswerReplace(question, index, count, text, expectedValue);
  return { ok: true, kind: 'answerReplace', result };
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
        await clearToken();
        // Also reset the bad-token block so the bridge returns to searching state.
        getClient().resetForNewToken();
        return { ok: true, kind: 'token' };
      }
      case 'reconnect': {
        await getClient().ensureConnected();
        return { ok: true, kind: 'status', status: await computeStatus() };
      }
      case 'import':
        return await runImport(req.applied);
      case 'fill':
        return await runFill();
      case 'appliedCheck':
        return await runAppliedCheck();
      case 'fieldsProbe':
        return await runFieldsProbe();
      case 'statusUpdate':
        return await runStatusUpdate();
      case 'answersSave':
        return await runAnswersSave();
      case 'answersSuggest':
        return await runAnswersSuggest();
      case 'answerFill':
        return await runAnswerFill(req.question, req.index, req.count, req.answer);
      case 'matchLive':
        return await runMatchLive();
      case 'answerAssist':
        return await runAnswerAssist(
          req.question,
          req.searchWeb,
          req.mode,
          req.existingAnswer,
          req.preset,
          req.instruction
        );
      case 'answerAssistProgress':
        return { ok: true, kind: 'answerAssistProgress', ...assistBuffer };
      case 'answerReplace':
        return await runAnswerReplace(
          req.question,
          req.index,
          req.count,
          req.text,
          req.expectedValue
        );
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
      injectSubmitWatch,
    });
  }
  return response;
}

// ── wiring ────────────────────────────────────────────────────────────────────

browser.runtime.onMessage.addListener(
  (message: unknown, sender: Browser.runtime.MessageSender): Promise<PopupResponse> | undefined => {
    // The injected submit-watcher posts a fire-and-forget `submitDetected` — it
    // is NOT a popup request and expects no response, so handle it out-of-band.
    if (isSubmitDetected(message)) {
      // Belt-and-braces MV3 hygiene: this extension declares no
      // `externally_connectable`, so no other extension/page can ever reach
      // this listener — but require the sender to be THIS extension anyway
      // before acting on it (defense-in-depth, costs nothing).
      if (sender.id === browser.runtime.id) {
        void handleSubmitDetected(message.url, submitFlowDeps());
      }
      return undefined;
    }
    return handleRequest(message as PopupRequest);
  }
);

// Re-probe on the lifecycle wake points so a freshly-started worker reconnects.
browser.runtime.onStartup.addListener(() => {
  void getClient().ensureConnected();
});
browser.runtime.onInstalled.addListener(() => {
  void getClient().ensureConnected();
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
