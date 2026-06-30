import { AlertTriangle } from 'lucide-react';
import { useId, useMemo } from 'react';
import { useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Alert, Dropdown } from '@ajh/ui';

import { StepTemplate } from '@/features/ai-generate/components/wizard-steps/StepTemplate';
import { OUTPUT_LANGUAGES, type TemplateId } from '@/lib/generate';

import type { BuilderFormValues } from '../../../types';
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

/** Recursively count leaf react-hook-form errors (those carrying a `message`). */
function countErrors(node: unknown): number {
  if (!node || typeof node !== 'object') return 0;
  if ('message' in (node as Record<string, unknown>)) return 1;
  return Object.values(node as Record<string, unknown>).reduce<number>(
    (sum, child) => sum + countErrors(child),
    0
  );
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
  const {
    formState: { errors },
  } = useFormContext<BuilderFormValues>();
  const errorCount = useMemo(() => countErrors(errors), [errors]);

  const isCjkLanguage = OUTPUT_LANGUAGES.some((l) => l.code === language && l.cjk);

  return (
    <div className="space-y-5">
      {!isComplete && <Alert type="warning" showIcon message={t('build.review.incomplete')} />}

      {errorCount > 0 && (
        <Alert type="error" showIcon message={t('build.review.fixIssues', { count: errorCount })} />
      )}

      <WizardField label={t('build.review.language')}>
        <Dropdown
          id={languageId}
          options={LANGUAGE_OPTIONS}
          value={language}
          onChange={onLanguageChange}
        />
      </WizardField>

      {isCjkLanguage && <Alert type="warning" showIcon message={t('build.review.cjkHint')} />}

      <StepTemplate
        templateId={templateId}
        atsMode={atsMode}
        onTemplateChange={onTemplateChange}
        onAtsModeChange={onAtsModeChange}
      />
    </div>
  );
}
