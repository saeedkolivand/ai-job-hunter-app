import type { ConfidenceLevel, ResumeField, StructuredResume } from '@ajh/shared';
import { Button, cn, GlassCard } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

interface Props {
  review: StructuredResume;
  onDismiss: () => void;
}

const CONFIDENCE_STYLE: Record<ConfidenceLevel, string> = {
  high: 'bg-emerald-500/15 text-emerald-400',
  medium: 'bg-amber-500/15 text-amber-400',
  low: 'bg-rose-500/15 text-rose-400',
};

function ConfidenceBadge({ level }: { level: ConfidenceLevel }) {
  const { t } = useTranslation();
  return (
    <span
      className={cn(
        'rounded-full px-2 py-0.5 text-[11px] font-medium leading-none',
        CONFIDENCE_STYLE[level]
      )}
    >
      {t(`resumeReview.confidence.${level}`)}
    </span>
  );
}

function FieldRow({ label, field }: { label: string; field?: ResumeField<string> }) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center justify-between gap-3 py-1.5">
      <span className="text-sm text-white/60">{label}</span>
      <span className="flex items-center gap-2">
        <span className={cn('text-sm', field ? 'text-foreground' : 'italic text-white/40')}>
          {field?.value || t('resumeReview.notDetected')}
        </span>
        {field && <ConfidenceBadge level={field.confidence} />}
      </span>
    </div>
  );
}

/**
 * Surfaces the structured-extraction review for a freshly imported resume:
 * detected contact fields with per-field confidence, the section inventory, and
 * any missing/low-confidence warnings. Informational — it never blocks; the user
 * dismisses it and proceeds.
 */
export function ResumeReviewPanel({ review, onDismiss }: Props) {
  const { t } = useTranslation();

  return (
    <GlassCard className="space-y-4 p-4">
      <header className="space-y-1">
        <h3 className="text-sm font-semibold text-foreground">{t('resumeReview.title')}</h3>
        <p className="text-xs text-white/60">{t('resumeReview.subtitle')}</p>
      </header>

      {review.warnings.length > 0 && (
        <ul className="space-y-1">
          {review.warnings.map((w) => (
            <li key={w} className="flex items-start gap-2 text-sm text-amber-400">
              <span aria-hidden className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-current" />
              <span>{w}</span>
            </li>
          ))}
        </ul>
      )}

      <div className="divide-y divide-white/10">
        <FieldRow label={t('resumeReview.name')} field={review.name} />
        <FieldRow label={t('resumeReview.email')} field={review.email} />
        <FieldRow label={t('resumeReview.phone')} field={review.phone} />
        <FieldRow label={t('resumeReview.location')} field={review.location} />
      </div>

      {review.links.length > 0 && (
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-white/60">{t('resumeReview.links')}</span>
          <div className="flex flex-wrap gap-1.5">
            {review.links.map((link) => (
              <span
                key={link.value}
                className="max-w-[14rem] truncate rounded-md bg-white/5 px-2 py-1 text-xs text-foreground"
                title={link.value}
              >
                {link.value}
              </span>
            ))}
          </div>
        </div>
      )}

      {review.sections.length > 0 && (
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-white/60">{t('resumeReview.sections')}</span>
          <div className="flex flex-wrap gap-1.5">
            {review.sections.map((section) => (
              <span
                key={`${section.kind}-${section.heading}`}
                className="flex items-center gap-1.5 rounded-md bg-white/5 px-2 py-1 text-xs text-foreground"
              >
                {section.heading}
                <ConfidenceBadge level={section.confidence} />
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="flex justify-end">
        <Button size="sm" variant="ghost" onClick={onDismiss}>
          {t('resumeReview.dismiss')}
        </Button>
      </div>
    </GlassCard>
  );
}
