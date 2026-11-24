import { Sparkles } from 'lucide-react';

import type { AutopilotNoteFit } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';
import { Tag } from '@ajh/ui';

const CATEGORY_COLOR = {
  supportedFit: 'success',
  stretch: 'warning',
  explicitGap: 'error',
  unknown: 'default',
} as const;

/**
 * The LLM-generated note on a found job / best match, with its deterministic fit beside it
 * (#1446): a category chip, the unmet and to-verify must-haves as lists (never folded into the
 * prose), and a "may be out of date" label when the résumé or posting changed since. Plain
 * text only; the note stays clamped to two lines with the full text in a tooltip.
 */
export function AiNote({ notes, fit }: { notes: string; fit: AutopilotNoteFit }) {
  const { t } = useTranslation();
  const lists = [
    { key: 'gaps', label: t('autopilot.fit.gapsLabel'), items: fit.mandatoryGaps ?? [] },
    { key: 'verify', label: t('autopilot.fit.verifyLabel'), items: fit.toVerify ?? [] },
  ].filter((l) => l.items.length > 0);
  return (
    <div
      role="note"
      aria-label={t('autopilot.aiNote')}
      className="ml-0.5 flex items-start gap-1.5 rounded-lg border border-brand/15 bg-brand/5 px-2.5 py-1.5"
    >
      <Sparkles size={10} className="mt-0.5 shrink-0 text-brand-soft" />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="block text-fine-print font-semibold uppercase tracking-wide text-brand-soft">
            {t('autopilot.aiNote')}
          </span>
          {fit.fitCategory && (
            <span data-testid="fit-category">
              <Tag color={CATEGORY_COLOR[fit.fitCategory]} className="px-1.5 py-0 text-[9px]">
                {t(`autopilot.fit.category.${fit.fitCategory}`)}
              </Tag>
            </span>
          )}
          {fit.notesStale && (
            <span className="text-[9px] italic text-foreground/70" data-testid="notes-stale">
              {t('autopilot.fit.stale')}
            </span>
          )}
        </div>
        <p title={notes} className="line-clamp-2 text-[10px] leading-relaxed text-foreground/70">
          {notes}
        </p>
        {lists.map((l) => (
          <p key={l.key} className="text-[10px] leading-relaxed text-foreground/70">
            <span className="font-semibold">{l.label}:</span> {l.items.join('; ')}
          </p>
        ))}
      </div>
    </div>
  );
}
