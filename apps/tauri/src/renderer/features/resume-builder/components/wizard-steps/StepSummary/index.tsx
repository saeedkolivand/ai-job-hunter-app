import { Controller, useFormContext } from 'react-hook-form';

import { TextArea } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import type { BuilderFormValues } from '../../../types';
import { WizardField } from '../../WizardField';

/**
 * Optional professional summary. If left blank, the synthesis derives one
 * strictly from the other answers; if filled, its substance is kept verbatim.
 */
export function StepSummary() {
  const { t } = useTranslation();
  const { control } = useFormContext<BuilderFormValues>();

  return (
    <WizardField
      label={t('build.summary.label')}
      hint={t('build.summary.hint')}
      htmlFor="build-summary"
    >
      <Controller
        control={control}
        name="summary"
        render={({ field }) => (
          <TextArea
            id="build-summary"
            variant="glass"
            value={field.value ?? ''}
            onChange={field.onChange}
            onBlur={field.onBlur}
            placeholder={t('build.summary.placeholder')}
            rows={5}
          />
        )}
      />
    </WizardField>
  );
}
