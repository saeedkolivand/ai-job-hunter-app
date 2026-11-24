import { TEST_IDS } from '@ajh/test-ids';
import { useTranslation } from '@ajh/translations';

/** Shown when the draft stage re-added the source résumé's Projects section (#1549). */
export function ProjectsRestoredNotice() {
  const { t } = useTranslation();
  return (
    <div
      data-testid={TEST_IDS.documents.projectsRestoredNotice}
      role="status"
      className="mx-8 mb-4 shrink-0 rounded-lg border border-[var(--border-clear)] bg-foreground/[0.02] px-4 py-3 text-[11px] text-foreground/60"
    >
      {t('autopilot.apply.projectsRestored')}
    </div>
  );
}
