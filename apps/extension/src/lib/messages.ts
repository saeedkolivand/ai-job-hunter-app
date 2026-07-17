/**
 * Internal popup ⇄ background message contract (NOT the wire protocol — that is
 * `@ajh/shared`'s extension-protocol). These travel over
 * `browser.runtime.sendMessage` inside the extension only.
 */

import type {
  ExtensionAnswerAssistResult,
  ExtensionAnswersSaveResult,
  ExtensionAnswersSuggestResult,
  ExtensionAppliedCheckResult,
  ExtensionImportResult,
  ExtensionMatchLiveResult,
  ExtensionRewritePreset,
  ExtensionStatusUpdateResult,
} from '@ajh/shared';

import type { FillAnswerResult } from './answer-fill';
import type { FilledField, ScannedQuestion } from './answers-capture';
import type { AutofillSummary } from './autofill';

/** Coarse connection state the popup renders. */
type ConnectionPhase =
  /** Background has not yet found a desktop bridge port. */
  | 'searching'
  /** A bridge port answered but no token is stored — show the pairing screen. */
  | 'not_paired'
  /** Paired + the v2 mutual handshake (incl. serverProof) with the desktop succeeded. */
  | 'connected'
  /** No bridge port answered in the probe range — the app is not running. */
  | 'app_not_running'
  /**
   * The desktop is too old to speak the v2 handshake (it never sent a
   * `challenge` — it closed the socket instead) — the user must UPDATE the
   * desktop app. Distinct from `bad_token` (a real token mismatch on a genuine
   * v2 desktop) and `app_not_running` (nothing answered at all).
   */
  | 'outdated'
  /** The stored token was rejected by the desktop — the user must re-pair. */
  | 'bad_token';

export interface ConnectionStatus {
  phase: ConnectionPhase;
  /** The bound port we connected to, when known (diagnostics only). */
  port: number | null;
  /** Whether a pairing token is currently stored. */
  hasToken: boolean;
}

/** popup → background requests. */
export type PopupRequest =
  | { kind: 'getStatus' }
  | { kind: 'setToken'; token: string }
  | { kind: 'clearToken' }
  | { kind: 'reconnect' }
  | { kind: 'import'; applied: boolean }
  /** Assisted autofill: fetch the profile fresh + inject the filler on this tab. */
  | { kind: 'fill' }
  /**
   * Fire-and-forget "have I already applied to this URL?" check for the
   * active tab, run once when the popup shows the connected view. Read-only;
   * never blocks the import controls.
   */
  | { kind: 'appliedCheck' }
  /**
   * Fire-and-forget "does this page have any fillable form fields?" probe,
   * run once when the popup shows the connected view — gates the Form group
   * and the Answer-tools disclosure (a page with no form, e.g. a plain job
   * listing, shows only the Job group). Read-only (counts candidate fields,
   * reads no values); like `appliedCheck`, never blocks the import controls.
   * Two independent signals come back — see `PopupResponse`'s `fieldsProbe`
   * doc for why they can't be one boolean.
   */
  | { kind: 'fieldsProbe' }
  /**
   * Fire-and-forget "is assisted autofill on?" read (Task #30), run
   * alongside `fieldsProbe` on entering the connected view — gates whether
   * the popup auto-runs "Suggest answers for this form" without a click.
   * Read-only, mirrors `fieldsProbe`'s never-blocks-the-import-controls
   * discipline exactly.
   */
  | { kind: 'autofillCheck' }
  /**
   * User-clicked "Mark as applied" for the active tab's URL. Unlike
   * `appliedCheck`, this is a deliberate WRITE action — its failures are
   * surfaced to the user, never folded away.
   */
  | { kind: 'statusUpdate' }
  /**
   * User-clicked "Save my answers from this page": capture the active tab's
   * filled form fields and send them on as `answers.save`. Like
   * `statusUpdate`, this is a deliberate WRITE action — its failures are
   * surfaced to the user, never folded away.
   */
  | { kind: 'answersSave' }
  /**
   * User-clicked "Suggest answers for this form": scan the active tab's
   * EMPTY candidate fields (questions mode) and send their labels on as
   * `answers.suggest`. Like `statusUpdate`, this is a deliberate action —
   * its failures are surfaced to the user, never folded away.
   */
  | { kind: 'answersSuggest' }
  /**
   * Per-row "Fill this field" click on one suggested answer. `question` +
   * `index` are the SAME scan-time correlation `answersSuggest` returned
   * (see `ScannedQuestion`); `count` is the total number of live fields that
   * shared this exact question text AT SCAN TIME. The filler re-locates the
   * exact field they name and fails safe if it can no longer find it OR if
   * the CURRENT same-question field count no longer matches `count` (e.g. a
   * same-labelled field inserted earlier in DOM order since the scan). Never
   * bulk, never submits the form.
   */
  | { kind: 'answerFill'; question: string; index: number; count: number; answer: string }
  /**
   * User-clicked "Check fit": capture the active tab's DOM (same Scan-mode
   * capture the import button uses) and send it as `match.live`. Explicit —
   * never runs automatically. Like `statusUpdate`, this is a deliberate
   * action — its failures are surfaced to the user, never folded away.
   */
  | { kind: 'matchLive' }
  /**
   * User-clicked "Help me answer…" (`mode` omitted/`'draft'`): draft a
   * paste-ready answer for a pasted or picked application question — the
   * first BILLABLE-AI verb on the bridge, gated on the SEPARATE AI-assist
   * opt-in (never the assisted-autofill one). `searchWeb` mirrors the in-app
   * toggle (default OFF).
   *
   * OR (extension PR 11) user-clicked a rewrite preset/submit
   * (`mode: 'rewrite'`): transform `existingAnswer` (the picked FILLED
   * field's current text) per `preset` or a free-text `instruction` — the
   * SAME billable opt-in, the SAME streaming path, but a PURE TEXT TRANSFORM
   * (never résumé/job/company-grounded, `searchWeb` is ignored). Like
   * `statusUpdate`, both modes are a deliberate action — failures are
   * surfaced to the user, never folded away.
   */
  | {
      kind: 'answerAssist';
      question: string;
      searchWeb: boolean;
      mode?: 'draft' | 'rewrite';
      existingAnswer?: string;
      preset?: ExtensionRewritePreset;
      instruction?: string;
    }
  /**
   * Popup-open reattach: "what's the current/last streamed `answer.assist`
   * text?" — the background OWNS the accumulation buffer (see
   * `PopupResponse`'s `answerAssistProgress` doc) so a popup that closed
   * mid-stream and reopens can immediately show what already arrived
   * instead of a blank view. Also pushed proactively (unsolicited, same
   * pattern as the `status` push) while a stream is running, live-updating
   * an OPEN popup.
   */
  | { kind: 'answerAssistProgress' }
  /**
   * Rewrite mode's Accept (write the AI-rewritten draft) / Restore original
   * (write the frozen pre-rewrite text) — SAME request, just a different
   * `text`; there is no separate "restore" kind. Mirrors `answerFill`'s
   * shape exactly: `question`/`index`/`count` are the picked field's OWN
   * scan-time correlation (from the SAME filled-fields scan
   * `answersSave`'s `filled` list carries — see `PopupResponse`), never
   * anything the background remembers on its own. `expectedValue` is what
   * the popup believes the field CURRENTLY holds (the frozen original at
   * pick time, or whatever a prior successful Accept/Restore wrote) —
   * `replaceFilledField` refuses (never clobbers) when the field's ACTUAL
   * current text no longer matches, i.e. the user edited it manually since
   * the pick. Locates + replaces via the same fail-safe re-scan `answerFill`
   * uses; never submits.
   */
  | {
      kind: 'answerReplace';
      question: string;
      index: number;
      count: number;
      text: string;
      expectedValue: string;
    };

/** background → popup responses (discriminated by the originating request). */
export type PopupResponse =
  | { ok: true; kind: 'status'; status: ConnectionStatus }
  | { ok: true; kind: 'token' }
  | { ok: true; kind: 'import'; result: ExtensionImportResult }
  | { ok: true; kind: 'fill'; summary: AutofillSummary }
  /**
   * Always `ok:true` — every failure mode (not paired, bridge down, malformed
   * reply, an old desktop's unrecognized message type) is folded into
   * `result.found === false` by the background, so the popup never has to
   * special-case an error path for this passive, best-effort check.
   */
  | { ok: true; kind: 'appliedCheck'; result: ExtensionAppliedCheckResult }
  /**
   * Always `ok:true` — like `appliedCheck`, EVERY failure (no active tab, a
   * restricted page, scripting permission denied) folds into `{hasFormFields:
   * true, hasAnswerFields: true}` (fail OPEN) so a probe bug can never hide
   * either feature — only a CONFIRMED empty scan hides them.
   *
   * TWO signals, not one: `hasFormFields` (gates the Form group — "Fill this
   * form" / "Save my answers") is the UNION of autofill-supported identity
   * fields (name/email/phone/…) and answer-capturable non-identity fields —
   * those two candidate sets are DISJOINT BY DESIGN (answers-capture
   * excludes identity fields, see `hasAnswerCapturableFields`'s doc), so a
   * single narrower boolean would wrongly hide "Fill this form" on a page
   * with ONLY identity fields. `hasAnswerFields` (gates the Answer-tools
   * disclosure — Suggest/rewrite) is the narrower answer-capturable-only
   * signal, since those tools have nothing to act on from identity fields
   * alone. See `probe-fields.ts` for where both are computed.
   */
  | { ok: true; kind: 'fieldsProbe'; hasFormFields: boolean; hasAnswerFields: boolean }
  /**
   * Always `ok:true` — mirrors `fieldsProbe`/`appliedCheck`'s never-a-
   * transport-error fold: `autofillEnabled()` itself never rejects (any
   * failure degrades to `false`, the safe default — see its doc on
   * `BridgeClient`).
   */
  | { ok: true; kind: 'autofillCheck'; enabled: boolean }
  /**
   * `ok:true` at the transport level; the desktop's own `ok`/`error` on
   * `result` is what the popup renders — this verb's failures are NOT
   * folded away (unlike `appliedCheck`), so the popup must check
   * `result.ok` itself.
   */
  | { ok: true; kind: 'statusUpdate'; result: ExtensionStatusUpdateResult }
  /**
   * `ok:true` at the transport level; like `statusUpdate`, the desktop's own
   * `ok`/`error` on `result` is what the popup renders — this verb's
   * failures are NOT folded away. `filled` (PR 11) is the CLIENT-SIDE
   * scan-time correlation for the rewrite picker — the SAME injection this
   * scan already ran (see `capture.ts`) — mirroring `answersSuggest`'s
   * `scanned`; an Accept/Restore later correlates back to a live field via
   * this exact `question`/`index`/count.
   */
  | { ok: true; kind: 'answersSave'; result: ExtensionAnswersSaveResult; filled: FilledField[] }
  /**
   * `ok:true` at the transport level; like `answersSave`, the desktop's own
   * `ok`/`error` on `result` is what the popup renders. `scanned` is the
   * CLIENT-SIDE scan-time correlation list (see `ScannedQuestion`) the popup
   * needs to know which suggestions have a live, fillable target field.
   */
  | {
      ok: true;
      kind: 'answersSuggest';
      result: ExtensionAnswersSuggestResult;
      scanned: ScannedQuestion[];
    }
  /** The per-row "Fill this field" outcome (fail-safe on any page mutation). */
  | { ok: true; kind: 'answerFill'; result: FillAnswerResult }
  /**
   * `ok:true` at the transport level; like `statusUpdate`, the desktop's own
   * `ok`/`error` on `result` is what the popup renders — this verb's
   * failures are NOT folded away.
   */
  | { ok: true; kind: 'matchLive'; result: ExtensionMatchLiveResult }
  /**
   * `ok:true` at the transport level; like `matchLive`, the desktop's own
   * `ok`/`error` on `result` is what the popup renders — this verb's
   * failures are NOT folded away.
   */
  | { ok: true; kind: 'answerAssist'; result: ExtensionAnswerAssistResult }
  /**
   * The CURRENT streaming `answer.assist` buffer, owned by the background
   * (not the popup) so it survives a popup close/reopen mid-stream —
   * `text` is the accumulated draft so far, `done` is true once the stream's
   * terminal reply arrived (success or failure), and `interrupted` is true
   * when the connection dropped (or the desktop returned an error)
   * mid-stream with SOME text already accumulated — the popup renders that
   * distinctly ("interrupted — here's what arrived") rather than a silent
   * hang or an empty error. Pushed proactively while a stream is running
   * (unsolicited, same pattern as the `status` push) AND returned on-demand
   * for the reattach case (`{ kind: 'answerAssistProgress' }`). `text` is
   * `''`/`done: true`/`interrupted: false` when no stream has ever run this
   * session.
   */
  | { ok: true; kind: 'answerAssistProgress'; text: string; done: boolean; interrupted: boolean }
  /** The rewrite Accept/Restore outcome (fail-safe on any page mutation) —
   *  mirrors `answerFill`'s response shape exactly. */
  | { ok: true; kind: 'answerReplace'; result: FillAnswerResult }
  | { ok: false; error: string };
