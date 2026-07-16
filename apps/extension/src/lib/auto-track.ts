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
 * Everything here is dependency-injected (no `browser` / `getClient` singletons)
 * so every branch is unit-testable with plain fakes.
 */

import type { ExtensionAppliedCheckResult, ExtensionStatusUpdateResult } from '@ajh/shared';

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
}

/**
 * Orchestrate a detected submit: RE-CHECK the opt-in (it may have been toggled
 * off since arming), then apply {@link decideSubmitAction}. Best-effort — every
 * failure (bridge unreachable, malformed reply) is swallowed so a page submit
 * never surfaces an error. The success confirmation for an auto-apply is shown
 * by the DESKTOP's own `status.update` notify tail (Notification Center + OS
 * banner), not here.
 */
export async function handleSubmitDetected(url: string, deps: SubmitFlowDeps): Promise<void> {
  try {
    const enabled = await deps.autotrackEnabled();
    if (!enabled) return;
    const applied = await deps.checkApplied(url);
    const action = decideSubmitAction(true, applied);
    if (action.kind === 'promptImport') deps.promptImport();
    else if (action.kind === 'autoApply') await deps.updateStatusAuto(url);
    // 'noop' → already applied / non-saved status → do nothing (silent).
  } catch {
    // Best-effort — never surface an error for a passive, page-triggered check.
  }
}

export interface ArmDeps {
  autotrackEnabled: () => Promise<boolean>;
  injectSubmitWatch: () => Promise<void>;
}

/**
 * Arm the submit watcher on the active tab — but ONLY when the auto-track
 * opt-in is on (the client-side gate; the desktop remains the authoritative
 * one). Best-effort: a restricted page, an unreachable bridge, or an unknown
 * opt-in all skip arming (the desktop still refuses any auto-write anyway).
 */
export async function maybeArmSubmitWatch(deps: ArmDeps): Promise<void> {
  try {
    if (!(await deps.autotrackEnabled())) return;
    await deps.injectSubmitWatch();
  } catch {
    // Best-effort — restricted page / bridge down / opt-in unknown.
  }
}
