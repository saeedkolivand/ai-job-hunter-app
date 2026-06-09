import { Info } from 'lucide-react';

import { SegmentedControl } from '@ajh/ui';

import { StepTemplate } from '@/features/ai-generate/components/wizard-steps/StepTemplate';
import type { TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { WizardField } from '../../WizardField';

interface StepReviewProps {
  language: string;
  templateId: TemplateId;
  atsMode: boolean;
  isComplete: boolean;
  onLanguageChange: (language: string) => void;
  onTemplateChange: (id: TemplateId) => void;
  onAtsModeChange: (enabled: boolean) => void;
}

/**
 * Final step: output language + template/ATS choice. The Generate button lives in
 * the wizard's top bar; this surfaces a hint when required sections are missing.
 */
export function StepReview({
  language,
  templateId,
  atsMode,
  isComplete,
  onLanguageChange,
  onTemplateChange,
  onAtsModeChange,
}: StepReviewProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-5">
      {!isComplete && (
        <div className="flex items-start gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3.5 py-2.5 text-xs text-amber-200/80">
          <Info size={14} className="mt-0.5 shrink-0" />
          <span>{t('build.review.incomplete')}</span>
        </div>
      )}

      <WizardField label={t('build.review.language')}>
        <SegmentedControl<string>
          variant="grid"
          ariaLabel={t('build.review.language')}
          value={language}
          onChange={onLanguageChange}
          options={[
            { value: 'en', label: t('build.review.languageEn') },
            { value: 'de', label: t('build.review.languageDe') },
          ]}
        />
      </WizardField>

      <StepTemplate
        templateId={templateId}
        atsMode={atsMode}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    </div>
  );
}
