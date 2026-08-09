import { useEffect, useRef } from 'react';

import { errorDetail } from '@/lib/error-class';
import {
  type GenerationMeta,
  mergeRecheckedReport,
  type QualityReport,
  serializeQualityReport,
} from '@/lib/generate';
import { useSaveAiGeneration } from '@/services/use-ai-generations';
import { useValidateContent } from '@/services/use-resume-validation';

export interface QualityRecheckParams {
  /** The session's current wrapper — the re-checked slot is merged into it. */
  report: QualityReport | null | undefined;
  /** Detected metadata of the run (supplies the validator's context). */
  meta: GenerationMeta | null;
  /** Source résumé the documents were tailored from. */
  sourceResume: string | undefined;
  /** Raw job ad. */
  jobAd: string | undefined;
  /** Which document the badge is showing — the one Re-check re-validates. */
  docKind: 'resume' | 'coverLetter';
  /** That document's CURRENT (possibly hand-edited) text. */
  currentText: string;
  /** Writes the merged wrapper back to session state. Omit to disable the
   *  action entirely (the returned `recheck` is then `undefined`). */
  onReportChange?: (report: QualityReport) => void;
  /** Both documents' current session text — persisted alongside the report so
   *  the stored text is the text the fresh hash describes. */
  resumeText: string;
  coverLetterText: string;
  /**
   * The saved generation's routing key. Persistence is SKIPPED without it —
   * see `persistReport` below.
   */
  jobUrl?: string;
  board?: string;
}

/**
 * Shared "Re-check" behaviour for the quality panel, used by every surface that
 * renders a `QualityBadge` over editable text (AI Generate's done panel and the
 * tailor flow's results panel).
 *
 * Re-validates the ACTIVE document's current text, merges the fresh slot into
 * the session's wrapper, and persists the result so the cleared staleness
 * survives a reopen. Best-effort throughout, exactly like `computeQualityReport`:
 * a validation or save failure leaves the stale badge showing rather than
 * opening a new error path for an optional action.
 */
export function useQualityRecheck({
  report,
  meta,
  sourceResume,
  jobAd,
  docKind,
  currentText,
  onReportChange,
  resumeText,
  coverLetterText,
  jobUrl,
  board,
}: QualityRecheckParams): { recheck: (() => void) | undefined; rechecking: boolean } {
  const validateContent = useValidateContent();
  const saveAiGeneration = useSaveAiGeneration();

  // Ownership guard: a session Reset (or a regeneration) clears `meta`/
  // `sourceResume`/`jobAd` while the validate call is still in flight — without
  // this, the stale result would resurrect a report into the freshly-cleared
  // session, or land on a document that has since been replaced. Bumped
  // whenever those identity-defining inputs change; mirrors the
  // AbortController ownership check `useGeneration`'s persist() uses.
  const epochRef = useRef(0);
  useEffect(() => {
    epochRef.current += 1;
  }, [meta, sourceResume, jobAd]);

  /**
   * Persist the merged wrapper WITHOUT disturbing anything else on the record.
   *
   * `save` is the only write path that reaches `quality_report` (a text
   * `update` deliberately never touches it), and it is a per-job merge-upsert:
   * Rust's `merge_application` picks each incoming field only when it is
   * non-blank (`pick = |inc, ex| if inc.trim().is_empty() { ex } else { inc }`;
   * empty vectors and the blank-language `mismatch` guard behave the same way),
   * so every field left blank here keeps its stored value. The two texts are
   * sent at their CURRENT session values — the exact strings the fresh hash was
   * computed over — so the reopened record can never read as stale by accident.
   *
   * Without a `jobUrl` there is no aggregate to merge onto: `find_by_job_url`
   * returns `None` for an empty url, so the save would INSERT a duplicate row
   * instead of updating the record the user is looking at. That surface keeps
   * the re-check session-only (deliberate) rather than forking the history.
   */
  const persistReport = (merged: QualityReport) => {
    if (!jobUrl) return;
    saveAiGeneration.mutate(
      {
        // Deliberately blank — the merge keeps whatever the record already holds.
        candidateName: '',
        jobTitle: '',
        companyName: '',
        resumeLanguage: '',
        jobAdLanguage: '',
        targetLanguage: '',
        mismatch: false,
        topRequirements: [],
        mode: '',
        jobAd: '',
        // Carried: the report, its routing key, and the text it describes.
        resumeText,
        coverLetterText,
        jobUrl,
        ...(board ? { board } : {}),
        qualityReport: serializeQualityReport(merged),
      },
      {
        onError: (err) =>
          console.warn('[useQualityRecheck] persisting the re-checked report failed', {
            error: errorDetail(err),
          }),
      }
    );
  };

  const handleRecheck = async () => {
    if (!sourceResume || !jobAd || !meta || !onReportChange) return;
    const epoch = epochRef.current;
    try {
      const payload = await validateContent.mutateAsync({
        generated: currentText,
        source: sourceResume,
        jobAd,
        topRequirements: meta.topRequirements,
        targetLanguage: meta.targetLanguage,
        docKind,
      });
      // The session moved on while this call was in flight — don't resurrect a
      // report onto whatever replaced it.
      if (epochRef.current !== epoch) return;
      const merged = mergeRecheckedReport(report ?? null, docKind, payload, currentText);
      onReportChange(merged);
      persistReport(merged);
    } catch (err) {
      console.warn('[useQualityRecheck] recheck failed — report left as-is', {
        docKind,
        error: errorDetail(err),
      });
    }
  };

  return {
    // No session writer means no way to show a result — hide the action.
    recheck: onReportChange ? () => void handleRecheck() : undefined,
    rechecking: validateContent.isPending,
  };
}
