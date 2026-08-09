import { CheckCircle2, ShieldAlert } from 'lucide-react';
import { useState } from 'react';

import { useTranslation } from '@ajh/translations';
import { Button } from '@ajh/ui';

import type { QualityReport } from '@/lib/generate';

import { QualityReportPanel } from './QualityReportPanel';

export interface QualityBadgeProps {
  /** The generation session's full report — resume + coverLetter portions. */
  report: QualityReport | null | undefined;
  /** Which document `docKind` this badge summarizes — selects `report.resume`
   *  or `report.coverLetter` and drives the panel's title. */
  docKind: 'resume' | 'coverLetter';
  className?: string;
}

/**
 * Compact, self-describing integrity chip: "Checked — no issues" or "N issues
 * (M critical)" for the active document. Clicking opens the full
 * {@link QualityReportPanel}. Renders nothing until this document has been
 * validated (before the first generation, or a doc `computeQualityReport`
 * never validated — e.g. a cover-only run's résumé tab).
 */
export function QualityBadge({ report, docKind, className }: QualityBadgeProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const docReport = docKind === 'resume' ? report?.resume : report?.coverLetter;

  if (!docReport) return null;

  const critical = docReport.issues.filter((issue) => issue.severity === 'critical').length;
  const clean = docReport.issues.length === 0;
  const label = clean
    ? t('quality.badge.clean')
    : t('quality.badge.issues', { count: docReport.issues.length, critical });

  return (
    <>
      <Button
        type="button"
        variant={clean ? 'success' : critical > 0 ? 'danger' : 'warning'}
        size="sm"
        onClick={() => setOpen(true)}
        title={t('quality.badge.open')}
        className={className}
      >
        {clean ? <CheckCircle2 size={11} /> : <ShieldAlert size={11} />}
        {label}
      </Button>
      <QualityReportPanel
        open={open}
        onClose={() => setOpen(false)}
        report={docReport}
        docKind={docKind}
      />
    </>
  );
}
