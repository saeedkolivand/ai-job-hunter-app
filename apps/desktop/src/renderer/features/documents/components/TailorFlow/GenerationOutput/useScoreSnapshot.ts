import { useEffect, useRef, useState } from 'react';

/**
 * Score-strip snapshot — lives in `GenerationOutput`, not inside `GenerationScoreStrip`,
 * because that strip only renders while `view === 'doc' && activeOut ===
 * 'resume'`: switching to the Job ad or Cover tab unmounts it. A `useState`
 * owned by the strip itself would re-initialise from whatever `jobDesc` is
 * live at remount — the Job ad sub-tab is an editable textarea one view
 * over, so "résumé tab → Job ad tab → edit posting → back" would silently
 * mint a fresh score for the edited text, including the translation-egress
 * cost `JobAdView`'s own snapshot (`scoreSnapshot`) exists to avoid. The
 * output component stays mounted across every tab switch, so the snapshot
 * survives there instead. Lazy-initializes from `jobDesc` to cover a cold-hydrated
 * session (`output` restored from a saved record with no `report` yet, so
 * the effect below never fires — the strip then scores against whatever
 * `jobDesc` the component mounted with, same as before the lift);
 * re-snapshots exactly once per NEW completed generation (`report`'s own
 * `generatedAt` changing).
 */
export function useScoreSnapshot(jobDesc: string, generatedAt: number | string | undefined) {
  const [scoreSnapshot, setScoreSnapshot] = useState(jobDesc);
  const lastScoreKeyRef = useRef(generatedAt);
  useEffect(() => {
    if (generatedAt !== undefined && generatedAt !== lastScoreKeyRef.current) {
      lastScoreKeyRef.current = generatedAt;
      setScoreSnapshot(jobDesc);
    }
  }, [generatedAt, jobDesc]);
  return scoreSnapshot;
}
