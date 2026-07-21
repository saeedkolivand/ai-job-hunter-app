import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';
import { Tag } from '@ajh/ui';

/**
 * Muted chip marking a posting whose company is a recruiting/staffing agency
 * (ADR-029 §i). Shared by the Jobs feature (row / compact row / detail pane) and
 * the Autopilot found-jobs list, so it lives outside both feature dirs. The
 * `data-testid` lives on the wrapper (`@ajh/ui` `Tag` takes no test id prop).
 */
export function AgencyChip({ className }: { className?: string }) {
  const { t } = useTranslation();
  return (
    <span data-testid={TEST_IDS.jobs.agencyChip} className="inline-flex">
      <Tag color="default" className={className}>
        {t('jobs.agencyChip')}
      </Tag>
    </span>
  );
}
