import { Info } from 'lucide-react';
import { useId } from 'react';

import type { InsightsVolumeNudge } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

/**
 * Calm heads-up about application volume. Plain labelled content, not a live
 * region: it is inserted already filled, so nothing would announce it. Not
 * dismissable, nothing persisted, never blocks anything. The advice is the takeaway.
 */
export function VolumeNudge({ nudge }: { nudge: InsightsVolumeNudge }) {
  const { t } = useTranslation();
  const headingId = useId();
  return (
    <section
      aria-labelledby={headingId}
      className="flex items-start gap-3 rounded-xl border border-[var(--border-clear)] bg-brand/5 px-4 py-3"
    >
      <Info size={16} aria-hidden="true" className="mt-0.5 shrink-0 text-brand-soft" />
      <div className="min-w-0 space-y-1 text-[12px] text-foreground/70">
        <h2 id={headingId} className="text-[12px] font-semibold text-foreground/85">
          {t('insights.nudge.title', {
            applications: nudge.applications,
            hours: nudge.windowHours,
          })}
        </h2>
        {nudge.triggers.includes('untailored') && (
          <p>
            {t('insights.nudge.untailored', {
              untailored: nudge.untailored,
              applications: nudge.applications,
            })}
          </p>
        )}
        {nudge.triggers.includes('nearCopyLetters') && (
          <p>
            {t('insights.nudge.nearCopy', {
              nearCopy: nudge.nearCopyLetters,
              letters: nudge.letters,
            })}
          </p>
        )}
        <p className="pt-1 text-[13px] text-foreground/85">{t('insights.nudge.advice')}</p>
      </div>
    </section>
  );
}
