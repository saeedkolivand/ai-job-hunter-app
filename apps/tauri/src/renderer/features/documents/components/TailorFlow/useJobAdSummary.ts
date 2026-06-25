import {
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
  useEffect,
  useRef,
  useState,
} from 'react';

import { generateJobAdSummary, type GenerationMeta } from '@/lib/generate';
import { useUpdateApplication } from '@/services/use-applications/use-applications';
import { useSessionStore } from '@/store/session-store';

interface Params {
  jobDesc: string;
  model: string;
  canUse: boolean;
  hasDesc: boolean;
  /** Metadata already detected by the tailor flow — passed through to the prompt. */
  meta?: GenerationMeta | null;
  /** The tracked Application id — used to persist the job summary onto the application record. */
  applicationId?: string;
  /** Persisted job summary from the application record (pre-seeds the summary panel). */
  initialSummary?: string;
}

interface RunParams {
  jobDesc: string;
  model: string;
  language: string;
  meta?: GenerationMeta | null;
  applicationId?: string;
  cacheKey: string;
  controller: AbortController;
  abortRef: MutableRefObject<AbortController | null>;
  summaryForDesc: MutableRefObject<string | null>;
  hasSummaryRef: MutableRefObject<boolean>;
  updateApplication: { mutate: (args: { id: string; jobSummary: string }) => void };
  setCachedJobSummary: (key: string, value: string) => void;
  setSummary: Dispatch<SetStateAction<string>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setGenerating: Dispatch<SetStateAction<boolean>>;
}

/**
 * Module-level runner so neither `generate()` nor the language-change effect
 * need to capture it as a reactive value, satisfying exhaustive-deps cleanly.
 */
function runGeneration({
  jobDesc,
  model,
  language,
  meta,
  applicationId,
  cacheKey,
  controller,
  abortRef,
  summaryForDesc,
  hasSummaryRef,
  updateApplication,
  setCachedJobSummary,
  setSummary,
  setError,
  setGenerating,
}: RunParams): Promise<void> {
  const target = jobDesc;
  return generateJobAdSummary({
    jobAd: target,
    meta,
    model,
    language,
    onToken: (tok) => setSummary((prev) => prev + tok),
    signal: controller.signal,
  })
    .then((result) => {
      if (controller.signal.aborted) return;
      setSummary(result);
      summaryForDesc.current = target;
      hasSummaryRef.current = true;
      if (applicationId) {
        updateApplication.mutate({ id: applicationId, jobSummary: result });
      } else {
        setCachedJobSummary(cacheKey, result);
      }
    })
    .catch((err: unknown) => {
      if (controller.signal.aborted) return;
      setError(err instanceof Error ? err.message : 'Failed to generate summary');
    })
    .finally(() => {
      if (abortRef.current === controller) {
        abortRef.current = null;
        setGenerating(false);
      }
    });
}

/**
 * Lazily streams an AI summary of the job ad. Résumé-independent. The component
 * triggers `generate()` on an explicit click — never auto-runs. The summary is
 * cached against the `jobDesc` it was produced from; when the user edits the ad
 * (jobDesc changes), the stale summary is dropped so a fresh one can be generated.
 *
 * When `applicationId` is set, successful results are persisted to the application
 * record via `useUpdateApplication`; otherwise they are stored in the session cache.
 *
 * Language auto-regenerate: when `language` changes AND a summary has already been
 * produced (or restored from cache/initialSummary), the hook aborts any in-flight
 * generation and starts a fresh one in the new language. If no summary exists yet,
 * the language change is stored silently and used on the next manual generate click.
 */
export function useJobAdSummary({
  jobDesc,
  model,
  canUse,
  hasDesc,
  meta,
  applicationId,
  initialSummary,
}: Params) {
  const updateApplication = useUpdateApplication();
  const setCachedJobSummary = useSessionStore((s) => s.setCachedJobSummary);
  // Session-cache fallback (used when there's no applicationId to persist onto):
  // restore a prior summary generated for THIS exact ad so re-entering the flow
  // starts populated instead of empty. Keyed by a jobDesc prefix (dedup-cheap).
  const cacheKey = jobDesc.trim().slice(0, 200);
  const cachedSummary = useSessionStore((s) => s.jobSummaryCache[cacheKey] ?? '');

  const [summary, setSummary] = useState(initialSummary ?? cachedSummary);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Locale CODE ('en', 'de', …) — must match an OUTPUT_LANGUAGES code so the
  // generation pipeline's safeLocale doesn't collapse the choice to English.
  const [language, setLanguage] = useState('en');
  // The jobDesc the current `summary` was generated from. When jobDesc diverges
  // from this, the summary is stale → reset it (so the empty state shows again).
  const summaryForDesc = useRef<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);
  // True once a summary has been successfully produced or restored (seed/cache).
  // Gate for the language auto-regenerate effect: changing language before a
  // summary has been produced just stores the choice silently.
  const hasSummaryRef = useRef(false);
  // Skips the language effect's mount invocation. `useEffect([language])` fires
  // once on mount; the seed effect (declared first) may have already set
  // hasSummaryRef=true for a restored/cached summary, which would cause the
  // mount run to re-generate and overwrite the restored text. This ref lets the
  // effect return early on mount and only react to real language changes.
  const languageSettled = useRef(false);

  // Stable ref holding the latest render-cycle values so the language-change
  // effect can read them without listing them as deps. Updated synchronously
  // during render so the effect always sees the current snapshot when it fires.
  const latestRef = useRef({
    jobDesc,
    model,
    canUse,
    hasDesc,
    meta,
    applicationId,
    cacheKey,
    updateApplication,
    setCachedJobSummary,
  });
  latestRef.current = {
    jobDesc,
    model,
    canUse,
    hasDesc,
    meta,
    applicationId,
    cacheKey,
    updateApplication,
    setCachedJobSummary,
  };

  // Seed summaryForDesc so the stale-desc reset doesn't clobber a restored summary
  // (persisted via initialSummary OR session-cached). jobDesc is intentionally
  // included: if the restored value arrives after jobDesc, we still stamp the
  // current desc so the stale-reset guard stays accurate.
  useEffect(() => {
    if ((initialSummary || cachedSummary) && summaryForDesc.current === null) {
      summaryForDesc.current = jobDesc;
      hasSummaryRef.current = true;
    }
  }, [initialSummary, cachedSummary, jobDesc]);

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
    hasSummaryRef.current = false;
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
    await runGeneration({
      jobDesc,
      model,
      language,
      meta,
      applicationId,
      cacheKey,
      controller,
      abortRef,
      summaryForDesc,
      hasSummaryRef,
      updateApplication,
      setCachedJobSummary,
      setSummary,
      setError,
      setGenerating,
    });
  };

  // Auto-regenerate when the user picks a different output language AND a summary
  // has already been produced or restored. `runGeneration` is module-level (not
  // a reactive value), so `language` is the only dep — fully satisfying
  // exhaustive-deps with no eslint-disable.
  useEffect(() => {
    // Skip the mount invocation. The seed effect (declared earlier) may have
    // already set hasSummaryRef=true for a restored/cached summary before this
    // effect fires, causing a spurious regeneration that overwrites the
    // restored text. Mark settled and return — only real language picks proceed.
    if (!languageSettled.current) {
      languageSettled.current = true;
      return;
    }

    const {
      canUse: cu,
      hasDesc: hd,
      jobDesc: jd,
      model: m,
      meta: mt,
      applicationId: appId,
      cacheKey: ck,
      updateApplication: upd,
      setCachedJobSummary: setCache,
    } = latestRef.current;
    // Skip if no summary exists yet — language change is a silent pick for later.
    // Skip if there is nothing to summarise or AI is unavailable.
    if (!hasSummaryRef.current || !jd.trim() || !cu || !hd) return;

    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    setGenerating(true);
    setError(null);
    setSummary('');

    void runGeneration({
      jobDesc: jd,
      model: m,
      language,
      meta: mt,
      applicationId: appId,
      cacheKey: ck,
      controller,
      abortRef,
      summaryForDesc,
      hasSummaryRef,
      updateApplication: upd,
      setCachedJobSummary: setCache,
      setSummary,
      setError,
      setGenerating,
    });

    return () => {
      controller.abort();
    };
  }, [language]); // runGeneration is module-level; all other values read via latestRef

  return { summary, generating, error, generate, language, setLanguage };
}
