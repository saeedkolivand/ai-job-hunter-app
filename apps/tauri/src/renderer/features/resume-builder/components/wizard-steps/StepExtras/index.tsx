import { Award, BookText, FolderGit2, HeartHandshake } from 'lucide-react';

import { Accordion, Input, TextArea } from '@ajh/ui';

import type { InterviewEntry, InterviewProject, InterviewPublication } from '@/lib/generate';
import { useTranslation } from '@/lib/i18n';

import type { BuilderStepProps } from '../../../types';
import { RepeatableList } from '../../RepeatableList';
import { WizardField } from '../../WizardField';

const blankEntry = (): InterviewEntry => ({ title: '', detail: '', year: '' });

/** Optional extra sections (projects, publications, awards, volunteering, languages, certs). */
export function StepExtras({ answers, update }: BuilderStepProps) {
  const { t } = useTranslation();

  return (
    <div className="space-y-2.5">
      <Accordion
        title={t('build.extras.projects.title')}
        content={
          <RepeatableList<InterviewProject>
            items={answers.projects ?? []}
            onChange={(projects) => update({ projects })}
            blank={() => ({ name: '', description: '', link: '' })}
            addLabel={t('build.extras.projects.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.projects.empty')}
            icon={FolderGit2}
            render={(item, set) => (
              <div className="space-y-2.5">
                <WizardField label={t('build.extras.projects.name')}>
                  <Input value={item.name} onChange={(e) => set({ name: e.target.value })} />
                </WizardField>
                <WizardField label={t('build.extras.projects.description')}>
                  <TextArea
                    value={item.description ?? ''}
                    onChange={(e) => set({ description: e.target.value })}
                    rows={2}
                  />
                </WizardField>
                <WizardField label={t('build.extras.link')} hint={t('build.extras.linkHint')}>
                  <Input value={item.link ?? ''} onChange={(e) => set({ link: e.target.value })} />
                </WizardField>
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.publications.title')}
        content={
          <RepeatableList<InterviewPublication>
            items={answers.publications ?? []}
            onChange={(publications) => update({ publications })}
            blank={() => ({ title: '', venue: '', year: '', link: '' })}
            addLabel={t('build.extras.publications.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.publications.empty')}
            icon={BookText}
            render={(item, set) => (
              <div className="space-y-2.5">
                <WizardField label={t('build.extras.publications.titleField')}>
                  <Input value={item.title} onChange={(e) => set({ title: e.target.value })} />
                </WizardField>
                <div className="grid grid-cols-2 gap-2.5">
                  <WizardField label={t('build.extras.publications.venue')}>
                    <Input
                      value={item.venue ?? ''}
                      onChange={(e) => set({ venue: e.target.value })}
                    />
                  </WizardField>
                  <WizardField label={t('build.extras.publications.year')}>
                    <Input
                      value={item.year ?? ''}
                      onChange={(e) => set({ year: e.target.value })}
                    />
                  </WizardField>
                </div>
                <WizardField label={t('build.extras.link')} hint={t('build.extras.linkHint')}>
                  <Input value={item.link ?? ''} onChange={(e) => set({ link: e.target.value })} />
                </WizardField>
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.awards.title')}
        content={
          <RepeatableList<InterviewEntry>
            items={answers.awards ?? []}
            onChange={(awards) => update({ awards })}
            blank={blankEntry}
            addLabel={t('build.extras.awards.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.awards.empty')}
            icon={Award}
            render={(item, set) => (
              <div className="grid grid-cols-[1fr_1fr_5rem] gap-2.5">
                <WizardField label={t('build.extras.entryTitle')}>
                  <Input value={item.title} onChange={(e) => set({ title: e.target.value })} />
                </WizardField>
                <WizardField label={t('build.extras.entryDetail')}>
                  <Input
                    value={item.detail ?? ''}
                    onChange={(e) => set({ detail: e.target.value })}
                  />
                </WizardField>
                <WizardField label={t('build.extras.entryYear')}>
                  <Input value={item.year ?? ''} onChange={(e) => set({ year: e.target.value })} />
                </WizardField>
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.volunteer.title')}
        content={
          <RepeatableList<InterviewEntry>
            items={answers.volunteer ?? []}
            onChange={(volunteer) => update({ volunteer })}
            blank={blankEntry}
            addLabel={t('build.extras.volunteer.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.volunteer.empty')}
            icon={HeartHandshake}
            render={(item, set) => (
              <div className="grid grid-cols-[1fr_1fr_5rem] gap-2.5">
                <WizardField label={t('build.extras.entryTitle')}>
                  <Input value={item.title} onChange={(e) => set({ title: e.target.value })} />
                </WizardField>
                <WizardField label={t('build.extras.entryDetail')}>
                  <Input
                    value={item.detail ?? ''}
                    onChange={(e) => set({ detail: e.target.value })}
                  />
                </WizardField>
                <WizardField label={t('build.extras.entryYear')}>
                  <Input value={item.year ?? ''} onChange={(e) => set({ year: e.target.value })} />
                </WizardField>
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.languages.title')}
        content={
          <WizardField
            label={t('build.extras.languages.label')}
            hint={t('build.extras.languages.hint')}
          >
            <TextArea
              value={(answers.languages ?? []).join('\n')}
              onChange={(e) => update({ languages: e.target.value.split('\n') })}
              rows={3}
              placeholder={t('build.extras.languages.placeholder')}
            />
          </WizardField>
        }
      />

      <Accordion
        title={t('build.extras.certifications.title')}
        content={
          <WizardField
            label={t('build.extras.certifications.label')}
            hint={t('build.extras.certifications.hint')}
          >
            <TextArea
              value={(answers.certifications ?? []).join('\n')}
              onChange={(e) => update({ certifications: e.target.value.split('\n') })}
              rows={3}
              placeholder={t('build.extras.certifications.placeholder')}
            />
          </WizardField>
        }
      />
    </div>
  );
}
