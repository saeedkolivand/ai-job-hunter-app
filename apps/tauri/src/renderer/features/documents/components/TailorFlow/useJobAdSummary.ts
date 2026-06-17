import { useEffect, useRef, useState } from 'react';

import { generateJobAdSummary, type GenerationMeta } from '@/lib/generate';

interface Params {
  jobDesc: string;
  model: string;
  canUse: boolean;
  hasDesc: boolean;
  /** Metadata already detected by the tailor flow — passed through to the prompt. */
  meta?: GenerationMeta | null;
}

/**
 * Lazily streams an AI summary of the job ad. Résumé-independent. The component
 * triggers `generate()` on an explicit click — never auto-runs. The summary is
 * cached against the `jobDesc` it was produced from; when the user edits the ad
 * (jobDesc changes), the stale summary is dropped so a fresh one can be generated.
 */
export function useJobAdSummary({ jobDesc, model, canUse, hasDesc, meta }: Params) {
  const [summary, setSummary] = useState('');
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The jobDesc the current `summary` was generated from. When jobDesc diverges
  // from this, the summary is stale → reset it (so the empty state shows again).
  const summaryForDesc = useRef<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Abort any in-flight generation on unmount so the stream is torn down and we
  // never setState on a dead component.
  useEffect(
    () => () => {
      abortRef.current?.abort();
    },
    []
  );

  if (summary && summaryForDesc.current !== null && summaryForDesc.current !== jobDesc) {
    // Render-phase reset: the ad was edited, so the cached summary no longer applies.
    setSummary('');
    setError(null);
    setGenerating(false);
    summaryForDesc.current = null;
  }

  const generate = async () => {
    if (!canUse || !hasDesc || !jobDesc.trim() || generating) return;
    // Restarting supersedes any prior in-flight stream so its tokens/result can't
    // bleed into the new run.
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    setGenerating(true);
    setError(null);
    setSummary('');
    const target = jobDesc;
    try {
      const result = await generateJobAdSummary({
        jobAd: target,
        meta,
        model,
        onToken: (tok) => setSummary((prev) => prev + tok),
        signal: controller.signal,
      });
      // A superseded/unmounted run must not clobber the current state.
      if (controller.signal.aborted) return;
      setSummary(result);
      summaryForDesc.current = target;
    } catch (err) {
      // An explicit abort (restart/unmount) is not an error to surface.
      if (!controller.signal.aborted) {
        setError(err instanceof Error ? err.message : 'Failed to generate summary');
      }
    } finally {
      if (abortRef.current === controller) {
        abortRef.current = null;
        setGenerating(false);
      }
    }
  };

  return { summary, generating, error, generate };
}
