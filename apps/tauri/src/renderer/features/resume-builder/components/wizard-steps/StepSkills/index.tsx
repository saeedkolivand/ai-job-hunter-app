import { Controller, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { TextArea } from '@ajh/ui';

import type { BuilderFormValues } from '../../../types';
import { WizardField } from '../../WizardField';

/** Skills (core section) — one per line; the synthesis groups them ATS-style. */
export function StepSkills() {
  const { t } = useTranslation();
  const { control } = useFormContext<BuilderFormValues>();

  return (
    <WizardField
      label={t('build.skills.label')}
      hint={t('build.skills.hint')}
      htmlFor="build-skills"
    >
      <Controller
        control={control}
        name="skills"
        render={({ field }) => (
          <TextArea
            id="build-skills"
            variant="glass"
            value={(field.value ?? []).join('\n')}
            onChange={(e) => field.onChange(e.target.value.split('\n'))}
            onBlur={field.onBlur}
            rows={6}
            placeholder={t('build.skills.placeholder')}
          />
        )}
      />
    </WizardField>
  );
}
