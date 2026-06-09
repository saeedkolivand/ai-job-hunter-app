import { Input, TextArea } from '@ajh/ui';

import type { InterviewEducation } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import type { BuilderStepProps } from '../../../types';
import { RepeatableList } from '../../RepeatableList';
import { WizardField } from '../../WizardField';

const blank = (): InterviewEducation => ({
  degree: '',
  institution: '',
  location: '',
  startDate: '',
  endDate: '',
  details: '',
});

/** Repeatable education entries (core section). */
export function StepEducation({ answers, update }: BuilderStepProps) {
  const { t } = useTranslation();

  return (
    <RepeatableList<InterviewEducation>
      items={answers.education ?? []}
      onChange={(education) => update({ education })}
      blank={blank}
      addLabel={t('build.education.add')}
      removeLabel={t('build.remove')}
      emptyLabel={t('build.education.empty')}
      render={(item, set) => (
        <div className="space-y-2.5">
          <div className="grid grid-cols-2 gap-2.5">
            <WizardField label={t('build.education.degree')}>
              <Input value={item.degree} onChange={(e) => set({ degree: e.target.value })} />
            </WizardField>
            <WizardField label={t('build.education.institution')}>
              <Input
                value={item.institution}
                onChange={(e) => set({ institution: e.target.value })}
              />
            </WizardField>
          </div>

          <div className="grid grid-cols-3 gap-2.5">
            <WizardField label={t('build.education.location')}>
              <Input
                value={item.location ?? ''}
                onChange={(e) => set({ location: e.target.value })}
              />
            </WizardField>
            <WizardField label={t('build.education.start')}>
              <Input
                value={item.startDate ?? ''}
                onChange={(e) => set({ startDate: e.target.value })}
              />
            </WizardField>
            <WizardField label={t('build.education.end')}>
              <Input
                value={item.endDate ?? ''}
                onChange={(e) => set({ endDate: e.target.value })}
              />
            </WizardField>
          </div>

          <WizardField label={t('build.education.details')} hint={t('build.education.detailsHint')}>
            <TextArea
              value={item.details ?? ''}
              onChange={(e) => set({ details: e.target.value })}
              rows={2}
            />
          </WizardField>
        </div>
      )}
    />
  );
}
