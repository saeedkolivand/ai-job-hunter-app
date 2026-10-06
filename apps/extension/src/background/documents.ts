/** Documents-into-ATS gestures: export a résumé/cover letter, attach or paste it. */

import type { ExtensionDocumentSource } from '@ajh/shared';

import type { AttachFileResult } from '../lib/attach-file';
import type { PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';
import { getClient, notPaired } from './bridge-client';
import { isAttachFileResult } from './guards';
import { activeTabId, activeTabOriginAtGesture, injectAndRun, tabStillConfirmed } from './page';

/** Isolated-world global key under which `attach-file.js` exposes the runner.
 *  MUST match `ATTACH_FILE_GLOBAL` in `lib/attach-file.ts`. */
const ATTACH_FILE_GLOBAL = '__ajhRunAttachFile';

/** Decode a base64 `document.result` payload to raw bytes — the one place that
 *  turns the wire string into bytes (`bridge.ts` stays base64-agnostic). */
function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * Export the picked source as DECODED plain text (cover letter, TXT only — the
 * picker's Copy/Paste actions both need text, never base64). A deliberate
 * click: failures are NOT folded away.
 */
export async function runDocumentExportText(
  source: ExtensionDocumentSource,
  templateId: string,
  letterLayoutId: string | undefined
): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();
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
 * The payload crosses the boundary as a base64 STRING, never a `Uint8Array`
 * (PR review round 2 — a real defect): Chrome JSON-serializes `executeScript`
 * `args`, so a `Uint8Array` arrived as `{"0":…}`, `new Uint8Array(that)` was
 * empty, and `attachResumeFile`'s byte-length verification always refused.
 * `attach-file.ts`'s `runAttachFile` decodes it back INSIDE the page.
 */
async function injectAttachFile(
  tabId: number,
  base64: string,
  filename: string,
  mimeType: string
): Promise<AttachFileResult> {
  const result = await injectAndRun(tabId, 'attach-file.js', ATTACH_FILE_GLOBAL, [
    base64,
    filename,
    mimeType,
  ]);
  if (!isAttachFileResult(result)) throw new Error('Could not attach the file on this page.');
  return result;
}

/**
 * "Attach résumé to this page": export as pdf/docx, inject, and surface the
 * fail-closed outcome. The caller (`documents/documents.ts`) owns the
 * first-time-per-site confirmation BEFORE sending this request. A deliberate
 * click: failures are NOT folded away.
 */
export async function runDocumentAttach(
  source: ExtensionDocumentSource,
  templateId: string,
  format: 'pdf' | 'docx',
  windowId?: number
): Promise<PopupResponse> {
  if (!(await getToken())) return notPaired();
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
