/** Import and assisted-autofill gestures, plus the passive fillable-fields probe. */

import type { ExtensionImportRequest } from '@ajh/shared';

import type { AutofillProfile, AutofillSummary } from '../lib/autofill';
import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { broadcastJobStatusChanged, getClient, notPaired } from './bridge-client';
import { isFieldsProbeResult, isFillSummary } from './guards';
import { activeTabUrl, captureTabHtml, injectAndRun, readPage, requireTabId } from './page';

/** Isolated-world global key under which `fill.js` exposes the filler. MUST
 *  match `AUTOFILL_GLOBAL` in `lib/autofill.ts` (pinned by a test there). */
const AUTOFILL_GLOBAL = '__ajhRunAutofill';

/** Run an import, always attempting to capture the rendered DOM first. */
export async function runImport(applied: boolean, windowId?: number): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();

  const url = await activeTabUrl(windowId);
  const payload: ExtensionImportRequest = { url, applied };
  // Always try to capture the authenticated DOM so the desktop can parse it
  // without re-fetching (which would hit bot-walls on LinkedIn/Indeed/Glassdoor).
  // Fall back to URL-only if executeScript is blocked (restricted pages).
  try {
    payload.html = await captureTabHtml(await requireTabId(windowId, 'No active tab to scan.'));
  } catch {
    // ponytail: restricted page or scripting permission denied — URL-only fallback
  }

  const result = await getClient().importJob(payload);
  // #1410: an open side panel must follow a popup-initiated save/applied.
  if (!result.error) void broadcastJobStatusChanged(url);
  return { ok: true, kind: 'import', result };
}

/** Inject the filler into the active tab and run it with `profile`. */
async function injectFill(profile: AutofillProfile, windowId?: number): Promise<AutofillSummary> {
  const tabId = await requireTabId(windowId, 'No active tab to fill.');
  const summary = await injectAndRun(tabId, 'fill.js', AUTOFILL_GLOBAL, [profile]);
  if (!isFillSummary(summary)) throw new Error('Could not fill the form on this page.');
  return summary;
}

/**
 * Assisted autofill: fetch the contact profile FRESH from the desktop (gated by
 * the desktop's opt-in — a refusal surfaces as an error) and inject the filler.
 * The profile is held only for this call and never persisted client-side.
 */
export async function runFill(windowId?: number): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();

  const profile = await getClient().getProfile();
  // Desktop refused (autofill off) or the reply was malformed — surface it.
  if (profile.error) return { ok: false, error: profile.error };

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
 * Passive "does this page have fillable form fields?" probe — gates the Form
 * group / Answer-tools disclosure. Never touches the bridge or the token, and
 * FAILS OPEN: any failure (no active tab, restricted page, scripting denied)
 * resolves BOTH signals `true` so a probe bug can never hide either feature —
 * only a CONFIRMED empty scan hides them.
 */
export async function runFieldsProbe(windowId?: number): Promise<PopupResponse> {
  try {
    const tabId = await requireTabId(windowId, 'No active tab to scan.');
    const { hasFormFields, hasAnswerFields } = await readPage(
      tabId,
      'probe-fields.js',
      isFieldsProbeResult,
      'Could not scan this page.'
    );
    return { ok: true, kind: 'fieldsProbe', hasFormFields, hasAnswerFields };
  } catch {
    return { ok: true, kind: 'fieldsProbe', hasFormFields: true, hasAnswerFields: true };
  }
}

/** Passive "is assisted autofill on?" read — `autofillEnabled()` never rejects
 *  (any failure degrades to `false`, the safe default), so no catch is needed. */
export async function runAutofillCheck(): Promise<PopupResponse> {
  const enabled = await getClient().autofillEnabled();
  return { ok: true, kind: 'autofillCheck', enabled };
}
