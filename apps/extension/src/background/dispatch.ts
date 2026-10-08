/** Central popup-request dispatcher: one switch over every `PopupRequest` kind. */

import type { PopupRequest, PopupResponse } from '../lib/messages';
import { looksLikeToken, setToken } from '../lib/storage';
import { assistProgress, runAnswerAssist, runAssistCancel } from './answer-assist';
import { runAnswerFill, runAnswerReplace } from './answer-fill';
import {
  runAnswerAccept,
  runAnswerAddRow,
  runAnswerRestoreOriginal,
  runAnswerRowAssist,
  runAnswerScan,
  runAnswerSelectVersion,
} from './answer-rows';
import { runAnswersSave, runAnswersSuggest } from './answers';
import { armSubmitWatch, clearImportPrompt } from './auto-track';
import { runAutofillCheck, runFieldsProbe, runFill, runImport } from './autofill';
import { computeStatus, getClient, unpairLocally } from './bridge-client';
import {
  runAppliedCheck,
  runAutoSaveNotice,
  runDocumentsList,
  runPrepGet,
  runProfileGet,
  runSettingsGet,
  runSettingsSet,
  runStatusUpdate,
  runTrustLineJob,
} from './desktop-queries';
import { runDocumentAttach, runDocumentExportText } from './documents';
import { runMatchLive } from './match-live';
import { runStampResults } from './results-stamp';

/** Popup requests whose handling injects a script into the active page — after
 *  a SUCCESSFUL one we arm the auto-track submit watcher (opt-in gated,
 *  idempotent per page). `stampResults` is deliberately EXCLUDED even though it
 *  injects a script: it is read-only (annotates a results page, no form
 *  interaction), so arming the watcher on a results page would let a later,
 *  unrelated submit-like interaction there auto-mark a saved application as
 *  applied (PR review finding). */
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

/** Never throws — maps errors to `ok:false`. */
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
          : await runAnswerAssist(req);
      case 'answerAssistProgress':
        return assistProgress();
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
        return await runAutoSaveNotice(req.windowId);
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
 * gesture — arm the auto-track submit watcher on that page, so a subsequent form
 * submit can auto-mark the matched application applied.
 */
export async function handleRequest(req: PopupRequest): Promise<PopupResponse> {
  const response = await dispatchRequest(req);
  if (response.ok && GESTURE_KINDS.has(req.kind)) armSubmitWatch(req.windowId);
  return response;
}
