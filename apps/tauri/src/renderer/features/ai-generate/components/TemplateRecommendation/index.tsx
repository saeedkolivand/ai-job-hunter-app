import { Check, Sparkles } from 'lucide-react';
import { useEffect, useState } from 'react';

import type { TemplateRecommendation as Recommendation } from '@ajh/shared';
import { Button } from '@ajh/ui';

import type { GenerationMeta, TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';
import { useRecommendTemplate } from '@/services';

interface Props {
  meta: GenerationMeta | null;
  templateId: TemplateId;
  onApply: (templateId: TemplateId, atsSuggested: boolean) => void;
}

/**
 * Shows the rules-based template suggestion once generation metadata is known
 * (job title, requirements, languages). Purely advisory — the user applies it
 * with one click or ignores it. The recommended locale is derived from the job
 * ad's language/country and printed in the rationale.
 */
export function TemplateRecommendation({ meta, templateId, onApply }: Props) {
  const { t } = useTranslation();
  const { mutate } = useRecommendTemplate();
  const [rec, setRec] = useState<Recommendation | null>(null);

  useEffect(() => {
    const hasSignals = Boolean(meta?.jobTitle) || (meta?.topRequirements?.length ?? 0) > 0;
    if (!hasSignals) {
      setRec(null);
      return;
    }
    mutate(
      {
        jobTitle: meta?.jobTitle,
        topRequirements: meta?.topRequirements,
        resumeLanguage: meta?.resumeLanguage,
        jobAdLanguage: meta?.jobAdLanguage,
      },
      { onSuccess: setRec }
    );
  }, [meta, mutate]);

  if (!rec) return null;

  const isCurrent = rec.templateId === templateId;

  return (
    <div className="mx-6 mb-3 rounded-lg border border-brand/20 bg-brand/5 px-3 py-2.5">
      <div className="flex items-start gap-2">
        <Sparkles size={13} className="mt-0.5 shrink-0 text-brand-soft" />
        <div className="min-w-0 flex-1 space-y-1.5">
          <p className="text-[11px] font-medium text-foreground/80">
            {t('aiGenerate.recommendationTitle')}
          </p>
          <p className="text-xs text-foreground/55">{rec.rationale}</p>
          {isCurrent ? (
            <span className="flex items-center gap-1 text-[11px] text-emerald-400">
              <Check size={11} /> {t('aiGenerate.recommendationApplied')}
            </span>
          ) : (
            <Button
              size="sm"
              variant="ghost"
              className="h-6 px-2 text-[11px] text-brand-soft hover:text-brand"
              onClick={() => onApply(rec.templateId, rec.atsSuggested)}
            >
              {t('aiGenerate.applyRecommendation')}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
