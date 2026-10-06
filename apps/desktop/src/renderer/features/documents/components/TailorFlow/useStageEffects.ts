import { useEffect, useRef, useState } from 'react';

import { useTranslation } from '@ajh/translations';

import type { TailorFlowStage } from './lib/tailor-stage';
import type { TailorRunState } from './ResultsPanel';

interface AnnouncementInput {
  stage: TailorFlowStage;
  error: string | null | undefined;
  state: string;
  runState: TailorRunState;
}

/**
 * CR-7: a `role="status"` element that only enters the DOM once its
 * condition is already true (the cancelled-no-output hint, ResultsPanel's
 * needsReview box) is unreliable — several screen readers only announce a
 * TEXT CHANGE inside an ALREADY-mounted live region, not content that arrives
 * in the same update as the region itself. The region this feeds stays
 * mounted for TailorFlow's whole lifetime (every stage transition); only its
 * text changes. It's additive, not a replacement — the two visual banners
 * keep their own `role="status"` too, for AT/browser combinations that DO
 * handle a freshly-mounted status role; this is the reliable fallback for the
 * ones that don't. Same "announce the TRANSITION, not the mount" posture as
 * GeneratingPanel's per-step announcer (H8).
 */
export function useLiveAnnouncement({ stage, error, state, runState }: AnnouncementInput) {
  const { t } = useTranslation();
  const [liveAnnouncement, setLiveAnnouncement] = useState('');
  const announcedKeyRef = useRef<'cancelled' | 'needsReview' | null>(null);
  useEffect(() => {
    const key =
      stage === 'configuring' && !error && state === 'cancelled'
        ? 'cancelled'
        : stage === 'done' && runState === 'needsReview'
          ? 'needsReview'
          : null;
    if (announcedKeyRef.current === key) return;
    announcedKeyRef.current = key;
    // CR-10: clearing to '' on the null branch (not leaving the previous
    // text in place) is the whole fix — for BOTH halves CodeRabbit raised.
    // Stale text: without this, a run that finishes cleanly after an
    // earlier cancel/needsReview kept exposing that old announcement in
    // the (still-mounted) region forever. Re-announcing an IDENTICAL
    // consecutive value: React bails out of a `setState` that's
    // `Object.is`-equal to the current value, so calling
    // `setLiveAnnouncement` with the SAME string twice in a row is not a
    // real DOM text mutation and may not re-announce — but `key` cannot
    // reach 'cancelled' (or 'needsReview') twice without passing through
    // `null` in between (starting a new run always moves `stage` off
    // 'configuring'/'done' first), so this clear always lands between two
    // occurrences of the same text, making the SECOND one a genuine ''→text
    // change again. No separate machinery needed for that half.
    if (key === 'cancelled') setLiveAnnouncement(t('autopilot.apply.cancelledNoOutput'));
    else if (key === 'needsReview') setLiveAnnouncement(t('pipeline.status.needsReview'));
    else setLiveAnnouncement('');
  }, [stage, error, state, runState, t]);
  return liveAnnouncement;
}

/**
 * `AnimatePresence mode="wait"` swaps the whole stage subtree on every stage
 * change, which drops focus to `<body>` with nothing to restore it —
 * keyboard/AT users lose their place after every wizard step, generate, or
 * "Edit settings". Focus the (otherwise inert, tabIndex={-1}) stage body
 * itself on each transition, matching a route/modal-swap pattern.
 *
 * Two guards, both load-bearing:
 * - Skip the FIRST run (mount): the ref below only tracks CHANGES, so a
 *   fresh page load never steals focus into an unlabeled offscreen div.
 * - Bail when focus is already inside an open `ModalShell` dialog
 *   (`[aria-modal="true"]`) — Questions/Interview/Referral stay open
 *   across a `generating → done` transition (ApplicationDetailPage keeps
 *   them enabled while busy), and `useFocusTrap` only intercepts Tab, not
 *   a programmatic `.focus()` landing outside the trap. Pulling focus out
 *   from under an open dialog would leave Tab walking the background page.
 */
export function useStageFocus(stage: TailorFlowStage) {
  const stageBodyRef = useRef<HTMLDivElement>(null);
  const mountedStageRef = useRef<TailorFlowStage | null>(null);
  useEffect(() => {
    const isFirstRun = mountedStageRef.current === null;
    mountedStageRef.current = stage;
    if (isFirstRun) return;
    const activeEl = stageBodyRef.current?.ownerDocument.activeElement;
    if (activeEl instanceof Element && activeEl.closest('[aria-modal="true"]')) return;
    stageBodyRef.current?.focus();
  }, [stage]);
  return stageBodyRef;
}
