import { Input, TextArea } from '@ajh/ui';

import type { InterviewExperience } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import type { BuilderStepProps } from '../../../types';
import { RepeatableList } from '../../RepeatableList';
import { WizardField } from '../../WizardField';

const blank = (): InterviewExperience => ({
  title: '',
  company: '',
  location: '',
  startDate: '',
  endDate: '',
  current: false,
  bullets: [],
});

/** Repeatable work-experience entries (core section). */
export function StepExperience({ answers, update }: BuilderStepProps) {
  const { t } = useTranslation();

  return (
    <RepeatableList<InterviewExperience>
      items={answers.experience ?? []}
      onChange={(experience) => update({ experience })}
      blank={blank}
      addLabel={t('build.experience.add')}
      removeLabel={t('build.remove')}
      emptyLabel={t('build.experience.empty')}
      render={(item, set) => (
        <div className="space-y-2.5">
          <div className="grid grid-cols-2 gap-2.5">
            <WizardField label={t('build.experience.title')}>
              <Input value={item.title} onChange={(e) => set({ title: e.target.value })} />
            </WizardField>
            <WizardField label={t('build.experience.company')}>
              <Input value={item.company} onChange={(e) => set({ company: e.target.value })} />
            </WizardField>
          </div>

          <div className="grid grid-cols-3 gap-2.5">
            <WizardField label={t('build.experience.location')}>
              <Input
                value={item.location ?? ''}
                onChange={(e) => set({ location: e.target.value })}
              />
            </WizardField>
            <WizardField label={t('build.experience.start')}>
              <Input value={item.startDate} onChange={(e) => set({ startDate: e.target.value })} />
            </WizardField>
            <WizardField label={t('build.experience.end')}>
              <Input
                value={item.endDate}
                onChange={(e) => set({ endDate: e.target.value })}
                disabled={item.current}
                placeholder={item.current ? t('build.experience.present') : ''}
              />
            </WizardField>
          </div>

          <label className="flex items-center gap-2 text-xs text-foreground/60">
            <input
              type="checkbox"
              checked={item.current ?? false}
              onChange={(e) => set({ current: e.target.checked })}
              className="accent-brand"
            />
            {t('build.experience.current')}
          </label>

          <WizardField
            label={t('build.experience.bullets')}
            hint={t('build.experience.bulletsHint')}
          >
            <TextArea
              value={(item.bullets ?? []).join('\n')}
              onChange={(e) => set({ bullets: e.target.value.split('\n') })}
              rows={4}
              placeholder={t('build.experience.bulletsPlaceholder')}
            />
          </WizardField>
        </div>
      )}
    />
  );
}
