import { AlertTriangle, Info } from 'lucide-react';
import { useId } from 'react';

import { SelectDropdown } from '@ajh/ui';

import { StepTemplate } from '@/features/ai-generate/components/wizard-steps/StepTemplate';
import { OUTPUT_LANGUAGES, type TemplateId } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import { WizardField } from '../../WizardField';

// CJK languages (zh/ja/ko) generate + export to DOCX fine, but the bundled Typst
// PDF/preview fonts can't render their glyphs yet — flag them in the picker.
const LANGUAGE_OPTIONS = OUTPUT_LANGUAGES.map(({ code, endonym, englishName, cjk }) => ({
  value: code,
  label: endonym === englishName ? englishName : `${englishName} · ${endonym}`,
  icon: cjk ? <AlertTriangle size={12} className="text-amber-300/70" /> : undefined,
}));

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
  const languageId = useId();

  const isCjkLanguage = OUTPUT_LANGUAGES.some((l) => l.code === language && l.cjk);

  return (
    <div className="space-y-5">
      {!isComplete && (
        <div className="flex items-start gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3.5 py-2.5 text-xs text-amber-200/80">
          <Info size={14} className="mt-0.5 shrink-0" />
          <span>{t('build.review.incomplete')}</span>
        </div>
      )}

      <WizardField label={t('build.review.language')}>
        <SelectDropdown
          id={languageId}
          options={LANGUAGE_OPTIONS}
          value={language}
          onChange={onLanguageChange}
        />
      </WizardField>

      {isCjkLanguage && (
        <div className="flex items-start gap-2 rounded-lg border border-amber-400/20 bg-amber-400/5 px-3.5 py-2.5 text-xs text-amber-200/80">
          <AlertTriangle size={14} className="mt-0.5 shrink-0" />
          <span>{t('build.review.cjkHint')}</span>
        </div>
      )}

      <StepTemplate
        templateId={templateId}
        atsMode={atsMode}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    </div>
  );
}
