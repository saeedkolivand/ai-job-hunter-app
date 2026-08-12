import { CheckCircle2, History, ShieldAlert } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import {
  type Fabrication,
  hashText,
  type QualityReport,
  removeEvidenceLines,
  unresolvedCount,
} from '@/lib/generate';

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
   * BADGE: while flagged claims are unsettled the run is `needsReview`, and
   * this chip must never read "no issues" for it — the document is usable but
   * unfinished, and that is the state the terminal review exists to surface.
   */
  pipeline?: QualityPipelineReview;
  /**
   * The host's ordinary document writer — the SAME one the editor calls, so a
   * removal lands in the surface's normal edit/save path (debounced commit,
   * preview refresh, persisted alongside a fresh report) instead of a private
   * side channel.
   *
   * Supplying it is what turns the review's "Remove" from a recorded intention
   * into an applied edit. Omit it (or pass `undefined` while the document is
   * locked/streaming) and Remove still records the verdict — the entry then
   * shows as "marked for removal", and the chip stays off green.
   */
  onDocumentTextChange?: (next: string) => void;
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
  onDocumentTextChange,
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
  // Unsettled flagged claims count as open issues here even when the
  // deterministic pass came back empty: the run is `needsReview`, and a green
  // "no issues" on it would be the exact misreport the review panel prevents.
  //
  // "Unsettled" is measured against `currentText`, not against the decision
  // alone — a Remove that was recorded but never applied leaves the claim
  // verbatim in the document, and that is not a clean run.
  const pendingClaims = pipeline ? unresolvedCount(pipeline.fabrications, currentText) : 0;
  const openIssues = docReport.issues.length + pendingClaims;
  const clean = openIssues === 0;
  const label = stale
    ? t('quality.badge.stale')
    : clean
      ? t('quality.badge.clean')
      : t('quality.badge.issues', { count: openIssues, critical });

  /**
   * Turn the host's text writer into the review's removal port. Built from
   * `currentText` — the live document, the same string staleness is measured
   * against — never from `pipeline.documentText`, which the host may be holding
   * at the run's snapshot; writing that back would undo the user's edits.
   *
   * An explicit `pipeline.onRemoveEvidence` wins, so a host with a different
   * apply strategy is not overridden.
   */
  const panelPipeline =
    pipeline && !pipeline.onRemoveEvidence && onDocumentTextChange
      ? {
          ...pipeline,
          onRemoveEvidence: (entry: Fabrication) => {
            const next = removeEvidenceLines(currentText, entry);
            // `null` = REFUSED, and the two reasons are indistinguishable from
            // here on purpose: the line is already gone (the verdict alone
            // settles the entry), or the entry has no anchor to delete by (the
            // verdict is recorded and the row says the line is still there).
            // Writing anything in that case is how the wrong line gets deleted.
            if (next !== null) onDocumentTextChange(next);
          },
        }
      : pipeline;

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
        pipeline={panelPipeline}
      />
    </>
  );
}
