import { TextArea } from '@ajh/ui';

import { useTranslation } from '@/lib/i18n';

import type { BuilderStepProps } from '../../../types';
import { WizardField } from '../../WizardField';

/**
 * Optional professional summary. If left blank, the synthesis derives one
 * strictly from the other answers; if filled, its substance is kept verbatim.
 */
export function StepSummary({ answers, update }: BuilderStepProps) {
  const { t } = useTranslation();

  return (
    <WizardField
      label={t('build.summary.label')}
      hint={t('build.summary.hint')}
      htmlFor="build-summary"
    >
      <TextArea
        id="build-summary"
        value={answers.summary ?? ''}
        onChange={(e) => update({ summary: e.target.value })}
        placeholder={t('build.summary.placeholder')}
        rows={5}
      />
    </WizardField>
  );
}
