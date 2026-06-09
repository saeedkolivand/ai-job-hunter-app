import { Input } from '@ajh/ui';

import { ContactProfileForm } from '@/components/contact/ContactProfileForm';
import { useTranslation } from '@/lib/i18n';

import type { BuilderStepProps } from '../../../types';
import { WizardField } from '../../WizardField';

/**
 * Contact + headline. Reuses the authoritative {@link ContactProfileForm} (it
 * saves to the contact profile, which becomes the exported header automatically),
 * and captures an optional headline that informs the synthesized summary.
 */
export function StepContact({ answers, update }: BuilderStepProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-5">
      <ContactProfileForm />

      <WizardField
        label={t('build.contact.headlineLabel')}
        hint={t('build.contact.headlineHint')}
        htmlFor="build-headline"
      >
        <Input
          id="build-headline"
          value={answers.headline ?? ''}
          onChange={(e) => update({ headline: e.target.value })}
          placeholder={t('build.contact.headlinePlaceholder')}
        />
      </WizardField>
    </div>
  );
}
