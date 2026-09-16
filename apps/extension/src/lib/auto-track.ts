/**
 * Auto-track (Task #22, Layer A) background decision + orchestration.
 *
 * After the injected `submit-watch.js` posts a detected form submit (see
 * `./submit-watch`), the background asks the desktop (over the bridge) whether
 * the just-submitted URL matches a tracked application and, if so, auto-marks it
 * `applied` — or nudges the user to import an untracked one. Both the arming and
 * this write are gated on the desktop-enforced auto-track opt-in; the write also
 * carries an `auto` flag the desktop re-gates server-side (defense-in-depth).
 *
 * PR4 (save answers on submit): a NESTED opt-in under auto-track — the
 * submit-watcher only captures answers at all when BOTH the auto-track AND
 * the `saveAnswersOnSubmit` opt-ins are on (see `maybeArmSubmitWatch`'s
 * `saveAnswersOnSubmitEnabled` dep), and any captured `answers` are saved
 * here only when the SAME `enabled` (auto-track) re-check above still passes
 * — so toggling auto-track off between arming and firing turns off both the
 * applied-check/auto-apply flow AND the answer save in one place, matching
 * the "nested" relationship. The desktop is still the decisive gate: it
 * refuses an `auto:true` `answers.save` unless `saveAnswersOnSubmit` is on
 * server-side too (mirrors `status_update.rs`'s `is_auto_status_update`/
 * `auto_write_refused` precedent) — this client-side nesting is defense in
 * depth only.
 *
 * Everything here is dependency-injected (no `browser` / `getClient` singletons)
 * so every branch is unit-testable with plain fakes.
 */

import type {
  ExtensionAnswersSaveResult,
  ExtensionAppliedCheckResult,
  ExtensionStatusUpdateResult,
} from '@ajh/shared';

import type { CapturedAnswer } from './answers-capture';

/** What to do on a detected submit — decided purely from the opt-in + the
 *  `applied.check` outcome. */
export type SubmitAction = { kind: 'autoApply' } | { kind: 'promptImport' } | { kind: 'noop' };

/**
 * Pure decision:
 *  - opt-in OFF → noop (belt-and-braces; the caller also gates before checking).
 *  - not tracked → prompt the user to import (NEVER auto-create).
 *  - tracked & currently `saved` → auto-mark applied.
 *  - tracked & already applied (or any other status) → noop.
 *
 * The `saved`-only auto-apply mirrors the desktop's `status.update` allowlist
 * (it performs ONLY `saved → applied`), so a job already past `saved`
 * (interviewing/offer/…) is never touched, and an already-`applied` job is a
 * silent no-op.
 */
export function decideSubmitAction(
  enabled: boolean,
  applied: ExtensionAppliedCheckResult
): SubmitAction {
  if (!enabled) return { kind: 'noop' };
  if (!applied.found) return { kind: 'promptImport' };
  if (applied.status === 'saved') return { kind: 'autoApply' };
  return { kind: 'noop' };
}

export interface SubmitFlowDeps {
  autotrackEnabled: () => Promise<boolean>;
  checkApplied: (url: string) => Promise<ExtensionAppliedCheckResult>;
  updateStatusAuto: (url: string) => Promise<ExtensionStatusUpdateResult>;
  promptImport: () => void;
  /** Save the captured `{question, answer}` pairs with `auto: true` (PR4).
   *  Only ever called when {@link handleSubmitDetected} was given a
   *  non-empty `answers` array. */
  saveAnswersAuto: (url: string, answers: CapturedAnswer[]) => Promise<ExtensionAnswersSaveResult>;
  /** Surface the transparent "here's what was saved, and where to change it"
   *  notice (PR4) — called ONLY on a successful auto-save, never on a
   *  refusal (nothing was saved, so nothing to announce; see this module's
   *  own doc for why the desktop's own gate can still refuse silently here). */
  notifyAutoSave: (result: Extract<ExtensionAnswersSaveResult, { ok: true }>) => void;
}

/**
 * Orchestrate a detected submit: RE-CHECK the opt-in (it may have been toggled
 * off since arming), then apply {@link decideSubmitAction}. Best-effort — every
 * failure (bridge unreachable, malformed reply) is swallowed so a page submit
 * never surfaces an error. The success confirmation for an auto-apply is shown
 * by the DESKTOP's own `status.update` notify tail (Notification Center + OS
 * banner), not here.
 *
 * `answers` (PR4) is the submit-watcher's own SYNCHRONOUS capture, present
 * only when it was armed with `captureAnswers: true` AND something was
 * filled — see `lib/submit-watch.ts`'s own doc for why that capture cannot
 * happen here (async, after the fact) at all. Saved through the SAME
 * best-effort try/catch as the applied-check/auto-apply flow above, and
 * gated behind the SAME `enabled` (auto-track) re-check — see this module's
 * doc for why the two are nested rather than independently gated.
 */
export async function handleSubmitDetected(
  url: string,
  deps: SubmitFlowDeps,
  answers?: CapturedAnswer[]
): Promise<void> {
  try {
    const enabled = await deps.autotrackEnabled();
    if (!enabled) return;
    const applied = await deps.checkApplied(url);
    const action = decideSubmitAction(true, applied);
    if (action.kind === 'promptImport') deps.promptImport();
    else if (action.kind === 'autoApply') await deps.updateStatusAuto(url);
    // 'noop' → already applied / non-saved status → do nothing (silent).
    if (answers && answers.length > 0) {
      const result = await deps.saveAnswersAuto(url, answers);
      // A refusal (the desktop's own saveAnswersOnSubmit gate off, or no
      // matched Application) degrades SILENTLY — nothing was saved, so
      // nothing to announce; never a partial claim.
      if (result.ok) deps.notifyAutoSave(result);
    }
  } catch {
    // Best-effort — never surface an error for a passive, page-triggered check.
  }
}

export interface ArmDeps {
  autotrackEnabled: () => Promise<boolean>;
  /** `captureAnswers` (PR4) — the desktop-enforced `saveAnswersOnSubmit`
   *  opt-in value, resolved by the caller BEFORE arming (see
   *  {@link maybeArmSubmitWatch}) and passed straight through as a plain
   *  boolean; the injected watcher never decides this policy itself. */
  injectSubmitWatch: (captureAnswers: boolean) => Promise<void>;
  /** Read the desktop-enforced save-answers-on-submit opt-in (PR4). Optional
   *  so an older/untouched caller (and every existing test) keeps arming
   *  WITHOUT capture, exactly as before — absent degrades to `false`, the
   *  safe default, same discipline as every other opt-in read here. */
  saveAnswersOnSubmitEnabled?: () => Promise<boolean>;
}

/**
 * Arm the submit watcher on the active tab — but ONLY when the auto-track
 * opt-in is on (the client-side gate; the desktop remains the authoritative
 * one). Best-effort: a restricted page, an unreachable bridge, or an unknown
 * opt-in all skip arming (the desktop still refuses any auto-write anyway).
 *
 * `saveAnswersOnSubmitEnabled` (PR4) is read in the SAME best-effort try —
 * a failure there degrades to `false` (capture off) rather than skipping
 * arming altogether, since the applied-check/auto-apply half of this feature
 * must not regress just because the newer, narrower opt-in couldn't be read.
 */
export async function maybeArmSubmitWatch(deps: ArmDeps): Promise<void> {
  try {
    if (!(await deps.autotrackEnabled())) return;
    let captureAnswers = false;
    try {
      captureAnswers = (await deps.saveAnswersOnSubmitEnabled?.()) ?? false;
    } catch {
      // Best-effort — arm without capture rather than not arm at all.
    }
    await deps.injectSubmitWatch(captureAnswers);
  } catch {
    // Best-effort — restricted page / bridge down / opt-in unknown.
  }
}
