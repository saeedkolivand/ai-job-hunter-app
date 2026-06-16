import { Controller, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';

import { ResumeInputCard } from '@/components/resume/ResumeInputCard';

import type { TailorWizardState } from '../../lib/tailor-state';

interface StepResumeProps {
  onUpload: (file: File) => Promise<void>;
  uploading: boolean;
}

/** Resume input step — the resume text is the only gated field of the wizard. */
export function StepResume({ onUpload, uploading }: StepResumeProps) {
  const { t } = useTranslation();
  const { control } = useFormContext<TailorWizardState>();

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.apply.wizard.resume.title')}
        </p>
        <p className="mt-0.5 text-xs text-foreground/35">
          {t('autopilot.apply.wizard.resume.subtitle')}
        </p>
      </div>

      <Controller
        control={control}
        name="resume"
        render={({ field, fieldState }) => (
          <div className="space-y-1.5">
            <ResumeInputCard
              value={field.value}
              onChange={field.onChange}
              onUpload={onUpload}
              uploading={uploading}
            />
            {fieldState.error?.message && (
              <p className="text-[10px] text-red-400/80">{t(fieldState.error.message)}</p>
            )}
          </div>
        )}
      />
    </div>
  );
}
