import { CheckCircle2, History, ShieldAlert } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { hashText, type QualityReport, unresolvedCount } from '@/lib/generate';

import { type QualityPipelineReview, QualityReportPanel } from './QualityReportPanel';

export interface QualityBadgeProps {
  /** The generation session's full report — resume + coverLetter slots. */
  report: QualityReport | null | undefined;
  /** Which document `docKind` this badge summarizes — selects `report.resume`
   *  or `report.coverLetter` and drives the panel's title. */
  docKind: 'resume' | 'coverLetter';
  /**
   * The document's CURRENT text — compared against the slot's own
   * `sourceTextHash` to detect a hand-edit (or a divergent cold-hydrated
   * record) since this report was generated. REQUIRED: every slot carries a
   * hash, so an omitted text would silently read as "the document was emptied"
   * and flag a permanent, bogus staleness.
   */
  currentText: string;
  /** Re-run validation against the current text — clears staleness on success.
   *  Omit to hide the panel's "Re-check" action entirely. */
  onRecheck?: () => void;
  rechecking?: boolean;
  className?: string;
  /**
   * Staged-run extras, forwarded to the panel. Their presence also changes the
   * BADGE: while flagged claims are undecided the run is `needsReview`, and
   * this chip must never read "no issues" for it — the document is usable but
   * unfinished, and that is the state the terminal review exists to surface.
   */
  pipeline?: QualityPipelineReview;
}

/**
 * Compact, self-describing integrity chip: "Checked — no issues" or "N issues
 * (M critical)" for the active document. Clicking opens the full
 * {@link QualityReportPanel}. Renders nothing until this document has been
 * validated (before the first generation, or a doc `computeQualityReport`
 * never validated — e.g. a cover-only run's résumé tab).
 *
 * A doc that has since diverged from the text this report validated (a
 * hand-edit, or a cold-hydrated record edited since it was saved) switches to
 * a distinct muted "checked before your edits" state — NEVER the green
 * "no issues" state on text the report never actually saw.
 */
export function QualityBadge({
  report,
  docKind,
  currentText,
  onRecheck,
  rechecking,
  className,
  pipeline,
}: QualityBadgeProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const slot = docKind === 'resume' ? report?.resume : report?.coverLetter;

  if (!slot) return null;

  // The verdict and the hash of the text it describes come from the SAME slot,
  // so they can never drift apart across a partial persistence merge.
  const docReport = slot.report;
  const stale = hashText(currentText) !== slot.sourceTextHash;

  const critical = docReport.issues.filter((issue) => issue.severity === 'critical').length;
  // Undecided flagged claims count as open issues here even when the
  // deterministic pass came back empty: the run is `needsReview`, and a green
  // "no issues" on it would be the exact misreport the review panel prevents.
  const pendingClaims = pipeline ? unresolvedCount(pipeline.fabrications) : 0;
  const openIssues = docReport.issues.length + pendingClaims;
  const clean = openIssues === 0;
  const label = stale
    ? t('quality.badge.stale')
    : clean
      ? t('quality.badge.clean')
      : t('quality.badge.issues', { count: openIssues, critical });

  return (
    <>
      <Button
        type="button"
        variant={stale ? 'info' : clean ? 'success' : critical > 0 ? 'danger' : 'warning'}
        size="sm"
        onClick={() => setOpen(true)}
        title={stale ? t('quality.badge.staleTooltip') : t('quality.badge.open')}
        className={className}
      >
        {stale ? (
          <History size={11} />
        ) : clean ? (
          <CheckCircle2 size={11} />
        ) : (
          <ShieldAlert size={11} />
        )}
        {label}
      </Button>
      <QualityReportPanel
        open={open}
        onClose={() => setOpen(false)}
        report={docReport}
        docKind={docKind}
        stale={stale}
        onRecheck={onRecheck}
        rechecking={rechecking}
        pipeline={pipeline}
      />
    </>
  );
}
