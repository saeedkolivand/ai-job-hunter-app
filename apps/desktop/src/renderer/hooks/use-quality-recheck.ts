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
  /** Writes the merged wrapper back to session state. Omit to disable the
   *  action entirely (the returned `recheck` is then `undefined`). */
  onReportChange?: (report: QualityReport) => void;
  /**
   * Both documents' CURRENT session text. `docKind` selects which of the two is
   * the text Re-check validates (they are the same two strings the surface
   * renders, so the validated text can never drift from the displayed one), and
   * both are persisted alongside the report so the stored text is the text the
   * fresh hash describes.
   */
  resumeText: string;
  coverLetterText: string;
  /**
   * `true` while THIS surface has a generation run in flight. Every transition
   * to `true` is a new run and ends the previous one's ownership — see the
   * epoch guard below.
   *
   * MUST be read from the component that OWNS the run state and stays mounted
   * across it (`AIGeneratePage`'s `isGenerating`, the tailor session's
   * `generating`), never from a panel the run unmounts: an unmounting child is
   * never re-rendered with the new value, so a guard keyed off it there would
   * be inert exactly when it matters.
   */
  generating: boolean;
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
 *
 * **Call it from the surface's state owner, not from a results panel.** The
 * ownership guarantees below all rest on this hook still re-rendering while a
 * re-check is in flight; a host that unmounts mid-run (AI Generate's
 * `OutputPanelDone` is swapped out the moment Regenerate sets stage
 * `generating`) freezes both the epoch and the live-session reads at their
 * pre-run values and silently loses every guarantee.
 */
export function useQualityRecheck({
  report,
  meta,
  sourceResume,
  jobAd,
  docKind,
  onReportChange,
  resumeText,
  coverLetterText,
  generating,
  jobUrl,
  board,
}: QualityRecheckParams): { recheck: (() => void) | undefined; rechecking: boolean } {
  const validateContent = useValidateContent();
  const saveAiGeneration = useSaveAiGeneration();

  const activeText = docKind === 'resume' ? resumeText : coverLetterText;

  /**
   * Ownership epoch. A re-check result belongs to ONE run of ONE document;
   * anything that starts a new run, or replaces the documents under it, has to
   * make an in-flight result unusable instead of letting it write back. Two
   * independent bumps, because neither subsumes the other:
   *
   * 1. **A new run started** — `generating` went false → true. This is the only
   *    thing that catches Regenerate: both `handleGenerate` (AI Generate) and
   *    `runTailor` (tailor flow) re-run with the SAME `meta`/`sourceResume`/
   *    `jobAd` values the user already had, so an identity-keyed guard alone
   *    never fires for them. Regenerate is also the worst case to miss: it
   *    clears the outputs and re-saves them, so a late merge would resurrect a
   *    report for text the user can no longer see, and a late `persistReport`
   *    would write the SUPERSEDED texts over the fresh ones (the save is a
   *    merge-upsert that takes any non-blank incoming text — it has no notion
   *    of which write is newer).
   * 2. **The inputs were replaced** — a session Reset clears `meta`/
   *    `sourceResume`/`jobAd` without ever setting `generating`.
   */
  const epochRef = useRef(0);
  useEffect(() => {
    epochRef.current += 1;
  }, [meta, sourceResume, jobAd]);
  const wasGeneratingRef = useRef(generating);
  useEffect(() => {
    if (generating && !wasGeneratingRef.current) epochRef.current += 1;
    wasGeneratingRef.current = generating;
  }, [generating]);

  /**
   * Live view of the session, refreshed on every commit. The post-await half of
   * `handleRecheck` reads THESE, never its own render-time captures:
   *
   * - the base `report` must be the one on screen when the result lands, or two
   *   re-checks of different documents overwrite each other's slot (the second
   *   to resolve would merge onto a base that predates the first);
   * - the two texts must be the live ones, so what gets persisted is what the
   *   fresh hash actually describes.
   */
  const liveRef = useRef({ report, resumeText, coverLetterText });
  useEffect(() => {
    liveRef.current = { report, resumeText, coverLetterText };
  }, [report, resumeText, coverLetterText]);

  /**
   * Persist the merged wrapper WITHOUT disturbing anything else on the record.
   *
   * `save` is the only write path that reaches `quality_report` (a text
   * `update` deliberately never touches it), and it is a per-job merge-upsert:
   * Rust's `merge_application` picks each incoming field only when it is
   * non-blank (`pick = |inc, ex| if inc.trim().is_empty() { ex } else { inc }`;
   * empty vectors and the blank-language `mismatch` guard behave the same way),
   * so every field left blank here keeps its stored value. The two texts come
   * from the caller at their LIVE values — the exact strings the fresh hash was
   * computed over — so the reopened record can never read as stale by accident.
   *
   * Without a `jobUrl` there is no aggregate to merge onto: `find_by_job_url`
   * returns `None` for an empty url, so the save would INSERT a duplicate row
   * instead of updating the record the user is looking at. That surface keeps
   * the re-check session-only (deliberate) rather than forking the history.
   */
  const persistReport = (merged: QualityReport, liveResume: string, liveCoverLetter: string) => {
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
        resumeText: liveResume,
        coverLetterText: liveCoverLetter,
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
    const validatedText = activeText;
    try {
      const payload = await validateContent.mutateAsync({
        generated: validatedText,
        source: sourceResume,
        jobAd,
        topRequirements: meta.topRequirements,
        targetLanguage: meta.targetLanguage,
        docKind,
      });
      // A newer run took over, or the inputs were replaced, while this call was
      // in flight — the result describes a document this surface no longer owns.
      if (epochRef.current !== epoch) return;
      const live = liveRef.current;
      const liveText = docKind === 'resume' ? live.resumeText : live.coverLetterText;
      // The same guarantee one level finer: the document changed underneath the
      // check (an edit landed, a run cleared it), so the fresh hash would
      // describe text that is no longer there. Drop it — never merge a report
      // with a hash that disagrees with the document it is attached to.
      if (liveText !== validatedText) return;
      const merged = mergeRecheckedReport(live.report ?? null, docKind, payload, validatedText);
      onReportChange(merged);
      persistReport(merged, live.resumeText, live.coverLetterText);
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
