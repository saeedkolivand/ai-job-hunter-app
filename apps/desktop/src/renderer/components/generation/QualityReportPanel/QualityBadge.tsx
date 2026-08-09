import { CheckCircle2, History, ShieldAlert } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import { hashText, type QualityReport } from '@/lib/generate';

import { QualityReportPanel } from './QualityReportPanel';

export interface QualityBadgeProps {
  /** The generation session's full report — resume + coverLetter portions. */
  report: QualityReport | null | undefined;
  /** Which document `docKind` this badge summarizes — selects `report.resume`
   *  or `report.coverLetter` and drives the panel's title. */
  docKind: 'resume' | 'coverLetter';
  /**
   * The document's CURRENT text — compared against `report.sourceTextHash` to
   * detect a hand-edit (or a divergent cold-hydrated record) since this report
   * was generated. Only needed when the report carries a hash; omit it (or a
   * legacy report with no hash) and staleness simply never triggers.
   */
  currentText?: string;
  /** Re-run validation against the current text — clears staleness on success.
   *  Omit to hide the panel's "Re-check" action entirely. */
  onRecheck?: () => void;
  rechecking?: boolean;
  className?: string;
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
  currentText = '',
  onRecheck,
  rechecking,
  className,
}: QualityBadgeProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const docReport = docKind === 'resume' ? report?.resume : report?.coverLetter;

  if (!docReport) return null;

  const validatedHash = report?.sourceTextHash?.[docKind];
  const stale = validatedHash !== undefined && hashText(currentText) !== validatedHash;

  const critical = docReport.issues.filter((issue) => issue.severity === 'critical').length;
  const clean = docReport.issues.length === 0;
  const label = stale
    ? t('quality.badge.stale')
    : clean
      ? t('quality.badge.clean')
      : t('quality.badge.issues', { count: docReport.issues.length, critical });

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
      />
    </>
  );
}
