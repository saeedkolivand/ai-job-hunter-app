/**
 * Background service worker / event page.
 *
 * Owns the single {@link BridgeClient} to the desktop loopback bridge and
 * answers the popup's `runtime.onMessage` requests. MV3 lifecycle: this context
 * can be evicted whenever idle, so all state is reconstructed lazily on wake
 * (`getClient()`), and we re-probe on `runtime.onStartup`, `onInstalled`, and
 * whenever the popup sends its first message.
 */

import { browser } from '@wxt-dev/browser';

import type { ExtensionImportRequest, ExtensionMatchLiveRequest } from '@ajh/shared';

// TYPE-ONLY import from the answer-fill module — same rationale as the
// autofill.ts import below: `answer-fill.js` is a classic-script injection
// target, so its runtime code must be imported ONLY by `answer-fill.ts`.
import type { FillAnswerResult } from './lib/answer-fill';
// Same type-only rationale as the autofill.ts import above — capture.js /
// capture-questions.js are ALSO classic-script injection targets (see
// vite.config.ts's `injectedEntries`), so this import must stay type-only
// (erased at build) to keep answers-capture.ts's runtime code out of the
// background's bundle.
import type { CapturedAnswer, ScannedQuestion } from './lib/answers-capture';
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
 *  `executeScript` boundary (capture.js's completion value). */
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

/**
 * Inject the answers-capture collector into the active tab and return its
 * `{question, answer}[]`. Single-step injection (unlike `injectFill`'s
 * files+func two-step): the collector takes no PII input to pass in
 * transiently, it only reads the page and returns data — same pattern as
 * `captureActiveTabHtml`.
 */
async function captureActiveTabAnswers(): Promise<CapturedAnswer[]> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to capture.');

  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['capture.js'],
  });
  const answers = results[0]?.result;
  if (!isCapturedAnswers(answers)) {
    throw new Error('Could not read the answers on this page.');
  }
  return answers;
}

/**
 * User-clicked "Save my answers from this page". UNLIKE `runAppliedCheck`,
 * failures are NOT folded away — a deliberate click action, like
 * `runStatusUpdate` — so a capture/transport failure propagates up to
 * `handleRequest`'s outer catch as `{ ok: false, error }`, and a resolved
 * desktop-side refusal (`{ ok: false, error }` — autofill off / no match /
 * malformed) still passes straight through as `result`. Mirrors `runFill`'s
 * not-paired short-circuit: the token check runs BEFORE the capture injection,
 * so an unpaired browser never reads the page.
 */
async function runAnswersSave(): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl();
  const answers = await captureActiveTabAnswers();
  const result = await getClient().saveAnswers(url, answers);
  return { ok: true, kind: 'answersSave', result };
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

/** Central popup-request dispatcher. Never throws — maps errors to `ok:false`. */
async function handleRequest(req: PopupRequest): Promise<PopupResponse> {
  try {
    switch (req.kind) {
      case 'getStatus': {
        // Opening the popup is a good moment to (re)probe the bridge.
        void getClient().ensureConnected();
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

// ── wiring ────────────────────────────────────────────────────────────────────

browser.runtime.onMessage.addListener((message: unknown): Promise<PopupResponse> =>
  handleRequest(message as PopupRequest)
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
