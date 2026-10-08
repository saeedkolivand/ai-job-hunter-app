import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';

/** Shown when `analyze_job` fell back to keyword-matched requirements (#1392). */
export function WeakAnalysisNotice() {
  const { t } = useTranslation();
  return (
    <div
      data-testid={TEST_IDS.documents.weakAnalysisNotice}
      role="status"
      className="mx-8 mb-4 shrink-0 rounded-lg border border-[var(--border-clear)] bg-foreground/[0.02] px-4 py-3 text-[11px] text-foreground/60"
    >
      {t('autopilot.apply.weakAnalysis')}
    </div>
  );
}
