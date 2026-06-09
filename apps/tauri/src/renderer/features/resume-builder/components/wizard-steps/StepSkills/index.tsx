import { TextArea } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import type { BuilderStepProps } from '../../../types';
import { WizardField } from '../../WizardField';

/** Skills (core section) — one per line; the synthesis groups them ATS-style. */
export function StepSkills({ answers, update }: BuilderStepProps) {
  const { t } = useTranslation();

  return (
    <WizardField
      label={t('build.skills.label')}
      hint={t('build.skills.hint')}
      htmlFor="build-skills"
    >
      <TextArea
        id="build-skills"
        value={(answers.skills ?? []).join('\n')}
        onChange={(e) => update({ skills: e.target.value.split('\n') })}
        rows={6}
        placeholder={t('build.skills.placeholder')}
      />
    </WizardField>
  );
}
