import { Controller, useFormContext } from 'react-hook-form';

import { Input } from '@ajh/ui';

import { ContactProfileForm } from '@/components/contact/ContactProfileForm';
import { useTranslation } from '@/lib/i18n';

import type { BuilderFormValues } from '../../../types';
import { WizardField } from '../../WizardField';

/**
 * Contact + headline. Reuses the authoritative {@link ContactProfileForm} (it
 * saves to the contact profile, which becomes the exported header automatically
 * and is NOT part of the builder form), and captures an optional headline (an
 * RHF field) that informs the synthesized summary.
 */
export function StepContact() {
  const { t } = useTranslation();
  const { control } = useFormContext<BuilderFormValues>();

  return (
    <div className="space-y-5">
      <ContactProfileForm />

      <WizardField
        label={t('build.contact.headlineLabel')}
        hint={t('build.contact.headlineHint')}
        htmlFor="build-headline"
      >
        <Controller
          control={control}
          name="headline"
          render={({ field }) => (
            <Input
              id="build-headline"
              className="w-full"
              value={field.value ?? ''}
              onChange={field.onChange}
              onBlur={field.onBlur}
              placeholder={t('build.contact.headlinePlaceholder')}
            />
          )}
        />
      </WizardField>
    </div>
  );
}
