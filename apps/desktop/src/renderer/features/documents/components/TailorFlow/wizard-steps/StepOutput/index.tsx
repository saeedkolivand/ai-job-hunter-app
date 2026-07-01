import { Controller, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { SegmentedControl } from '@ajh/ui';

import type { TailorWizardState } from '../../lib/tailor-state';
import type { TailorTarget } from '../../useTailorGeneration';

/** Output-type step — resume, cover letter, or both. Bounded control, no gate. */
export function StepOutput() {
  const { t } = useTranslation();
  const { control } = useFormContext<TailorWizardState>();

  const options: { value: TailorTarget; label: string }[] = [
    { value: 'resume', label: t('autopilot.apply.target.resume') },
    { value: 'cover', label: t('autopilot.apply.target.cover') },
    { value: 'both', label: t('autopilot.apply.target.both') },
  ];

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm font-semibold text-foreground/70">
          {t('autopilot.apply.wizard.output.title')}
        </p>
        <p className="mt-0.5 text-xs text-foreground/35">
          {t('autopilot.apply.wizard.output.subtitle')}
        </p>
      </div>

      <Controller
        control={control}
        name="outputType"
        render={({ field }) => (
          <SegmentedControl<TailorTarget>
            ariaLabel={t('autopilot.apply.target.label')}
            value={field.value}
            onChange={field.onChange}
            options={options}
            variant="grid"
          />
        )}
      />
    </div>
  );
}
