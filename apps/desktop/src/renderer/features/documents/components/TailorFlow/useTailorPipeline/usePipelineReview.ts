import { useMemo } from 'react';

import { useTranslation } from '@ajh/translations';

import type { QualityPipelineReview } from '@/components/generation/QualityReportPanel';
import type { ResumePipelineSession } from '@/hooks/use-resume-pipeline-session';
import { errorDetail } from '@/lib/error-class';
import { buildSectionVerdicts, parseFabrications, unresolvedCount } from '@/lib/generate';
import { useRegenerateSection, useResolveFabrication } from '@/services/use-resume-pipeline';

interface Params {
  session: ResumePipelineSession;
  activeOut: 'resume' | 'cover';
  /** See `activeIsThisRunsOwn` in `useTailorPipeline`. */
  activeIsThisRunsOwn: boolean;
  output: string;
  resumeOut: string;
  coverOut: string;
}

/**
 * Section-fix / fabrication-review extras for the ACTIVE document — this
 * session's OWN run is always the posting's newest (nothing else can start
 * one from here), so unlike `TailoredResumePanel` there is no older-run
 * gate to apply.
 */
export function usePipelineReview({
  session,
  activeOut,
  activeIsThisRunsOwn,
  output,
  resumeOut,
  coverOut,
}: Params) {
  const { t } = useTranslation();
  const regenerate = useRegenerateSection();
  const resolveFabrication = useResolveFabrication();

  // Read off the RAW `PipelineQualityReport` (not the renderer-shaped `report`)
  // — its slot type declares `fabrications`, where `QualityReportSlot`
  // deliberately doesn't (it's opaque additional data there).
  const rawSlot = !activeIsThisRunsOwn
    ? undefined
    : activeOut === 'resume'
      ? session.detail?.report?.resume
      : session.detail?.report?.coverLetter;
  const sections = useMemo(() => buildSectionVerdicts(rawSlot?.report, output), [rawSlot, output]);
  const fabrications = useMemo(() => parseFabrications(rawSlot?.fabrications), [rawSlot]);
  const runId = session.detail?.runId;

  // Unresolved fabrication count across BOTH documents — the Rust
  // `needsReview` verdict (`still_needs_review`) scans resume AND coverLetter,
  // but `fabrications` above (and the ACTIVE-tab-only `pipelineReview` it
  // feeds) only ever reflects whichever document is on screen. A run flagged
  // for review while the user is looking at the OTHER, clean document must
  // not read as "0 claims" just because this session hasn't switched tabs.
  const resumeReportSlot = session.detail?.report?.resume;
  const coverReportSlot = session.detail?.report?.coverLetter;
  const openClaimsTotal = useMemo(() => {
    const resumeUnresolved = resumeReportSlot
      ? unresolvedCount(parseFabrications(resumeReportSlot.fabrications), resumeOut)
      : 0;
    const coverUnresolved = coverReportSlot
      ? unresolvedCount(parseFabrications(coverReportSlot.fabrications), coverOut)
      : 0;
    return resumeUnresolved + coverUnresolved;
  }, [resumeReportSlot, coverReportSlot, resumeOut, coverOut]);
  const pipelineReview: QualityPipelineReview | undefined =
    runId && activeIsThisRunsOwn
      ? {
          documentText: output,
          sections,
          fabrications,
          onFixSection: (sectionKey, note) =>
            regenerate.mutate({ runId, sectionKey, ...(note ? { note } : {}) }),
          fixingSection: regenerate.isPending ? (regenerate.variables?.sectionKey ?? null) : null,
          fixError: regenerate.error
            ? t('autopilot.apply.wizard.results.fixFailed', {
                detail: errorDetail(regenerate.error),
              })
            : null,
          onResolveFabrication: (issueKey, decision) =>
            resolveFabrication.mutate({ runId, issueKey, decision }),
          resolvingIssueKey: resolveFabrication.isPending
            ? (resolveFabrication.variables?.issueKey ?? null)
            : null,
          resolveError: resolveFabrication.error
            ? t('autopilot.apply.wizard.results.resolveFailed', {
                detail: errorDetail(resolveFabrication.error),
              })
            : null,
          ...(session.detail?.metrics.repairRounds != null
            ? { repairRounds: session.detail.metrics.repairRounds }
            : {}),
          ...(session.detail?.metrics.reverted != null
            ? { repairReverted: session.detail.metrics.reverted }
            : {}),
        }
      : undefined;

  return { openClaimsTotal, pipelineReview };
}
