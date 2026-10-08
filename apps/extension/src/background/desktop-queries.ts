/**
 * Thin popup-request handlers that ask the desktop about the active tab's job
 * (or the extension's settings) and relay the answer. Passive reads fold every
 * failure into an empty answer; deliberate clicks (`statusUpdate`, settings,
 * the documents/prep tabs) do NOT — the user must see why they failed.
 */

import type { ExtensionSettingsKey } from '@ajh/shared';

import { takeAutoSaveNotice } from '../lib/auto-save-notice';
import { stripFenceWrapper } from '../lib/fence-strip';
import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { broadcastJobStatusChanged, getClient } from './bridge-client';
import { activeTabIn, activeTabUrl } from './page';

/**
 * Fire-and-forget "have I already applied?" check — a read-only, best-effort
 * enhancement over the import view. NEVER surfaces `ok:false`: any failure
 * (not paired, bridge unreachable, an old desktop's unrecognized message type,
 * a malformed reply) folds into `{ found: false }`.
 */
export async function runAppliedCheck(windowId?: number): Promise<PopupResponse> {
  try {
    const url = await activeTabUrl(windowId);
    const result = await getClient().checkApplied(url);
    return { ok: true, kind: 'appliedCheck', result };
  } catch {
    return { ok: true, kind: 'appliedCheck', result: { found: false } };
  }
}

/**
 * Job tab copy-field fallback (decision 8): the Contact Profile, fetched fresh
 * through the same Autofill opt-in gate `runFill` uses. Passive: NEVER surfaces
 * `ok:false` — any failure folds into `result.error`. Held only for this call.
 */
export async function runProfileGet(): Promise<PopupResponse> {
  try {
    if (!(await getToken())) {
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

/** The `job` resource's `data` shape — only the two fields the trust line
 *  renders; `title`/`company` are optional strings on the wire. */
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
 * Passive "what does the read tier say about this page's job?" lookup feeding
 * the side panel's trust line. ANY refusal (Autofill off, throttled, an
 * unknown job, no connection) resolves both fields `null`, which the panel
 * renders as its existing host-only line.
 */
export async function runTrustLineJob(windowId?: number): Promise<PopupResponse> {
  try {
    const url = await activeTabUrl(windowId);
    const res = await getClient().agentQuery('job', { url });
    if (!res.ok) return { ok: true, kind: 'trustLineJob', title: null, company: null };
    return { ok: true, kind: 'trustLineJob', ...readJobTitleCompany(res.data) };
  } catch {
    return { ok: true, kind: 'trustLineJob', title: null, company: null };
  }
}

/** Settings page: read the opt-in switches. Failures propagate to the
 *  dispatcher's outer catch; a resolved desktop refusal passes through as
 *  `result` so the page can show why the toggles couldn't load. */
export async function runSettingsGet(): Promise<PopupResponse> {
  const result = await getClient().settingsGet();
  return { ok: true, kind: 'settingsGet', result };
}

/** Settings page: flip one switch (the page rolls its optimistic toggle back on
 *  a well-formed `ok:false`). */
export async function runSettingsSet(
  key: ExtensionSettingsKey,
  enabled: boolean
): Promise<PopupResponse> {
  const result = await getClient().settingsSet(key, enabled);
  return { ok: true, kind: 'settingsSet', result };
}

/**
 * User-clicked "Mark as applied" for the active tab's URL. A deliberate click:
 * a transport rejection propagates to the dispatcher's outer catch as
 * `{ ok: false, error }`, and a resolved desktop refusal (no match / wrong
 * starting status / unsupported transition) still passes through as `result`.
 */
export async function runStatusUpdate(windowId?: number): Promise<PopupResponse> {
  const url = await activeTabUrl(windowId);
  const result = await getClient().updateStatus(url);
  // #1410: an open side panel must follow a popup-initiated flip.
  if (result.ok) void broadcastJobStatusChanged(url);
  return { ok: true, kind: 'statusUpdate', result };
}

/**
 * Documents tab: list this job's generation + saved base résumés (the curated
 * `documents` read-tier resource). A refusal is NOT folded away — the tab
 * renders the desktop's own `error`. `url` is echoed back so the tab (which has
 * no `tabs` permission of its own) can build its sources without a second
 * round trip.
 */
export async function runDocumentsList(windowId?: number): Promise<PopupResponse> {
  const url = await activeTabUrl(windowId).catch(() => '');
  const result = await getClient().agentQuery('documents', { url });
  return { ok: true, kind: 'documentsList', result, url };
}

/** Prep tab: this job's existing generations (company brief, interview
 *  questions, salary answer) — same shape and fold-nothing discipline as
 *  {@link runDocumentsList}. */
export async function runPrepGet(windowId?: number): Promise<PopupResponse> {
  const url = await activeTabUrl(windowId).catch(() => '');
  const result = await getClient().agentQuery('prep', { url });
  return { ok: true, kind: 'prepGet', result, url };
}

/** Read-once "was there a transparent save-answers-on-submit notice waiting?"
 *  (see `lib/auto-save-notice.ts`). */
export async function runAutoSaveNotice(windowId?: number): Promise<PopupResponse> {
  const tab = await activeTabIn(windowId).catch(() => undefined);
  const text = await takeAutoSaveNotice(tab?.id);
  return { ok: true, kind: 'autoSaveNotice', text };
}
