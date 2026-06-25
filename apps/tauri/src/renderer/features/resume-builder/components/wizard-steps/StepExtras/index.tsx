import { Award, BookText, FolderGit2, GitBranch, HeartHandshake } from 'lucide-react';
import { useState } from 'react';
import { Controller, useFieldArray, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Accordion, Button, Input, TextArea } from '@ajh/ui';

import type { BuilderFormValues } from '../../../types';
import { FieldArrayList } from '../../FieldArrayList';
import { GitHubImportModal } from '../../GitHubImportModal';
import { WizardField } from '../../WizardField';

/** Optional extra sections (projects, publications, awards, volunteering, languages, certs). */
export function StepExtras() {
  const { t } = useTranslation();
  const { control, formState } = useFormContext<BuilderFormValues>();
  const { errors } = formState;
  const [githubOpen, setGithubOpen] = useState(false);

  // Translate an i18n-key error message (the schema stores keys) or pass through undefined.
  const msg = (key: string | undefined) => (key ? t(key) : undefined);

  const projects = useFieldArray({ control, name: 'projects' });
  const publications = useFieldArray({ control, name: 'publications' });
  const awards = useFieldArray({ control, name: 'awards' });
  const volunteer = useFieldArray({ control, name: 'volunteer' });

  return (
    <div className="space-y-2.5">
      <GitHubImportModal
        open={githubOpen}
        onClose={() => setGithubOpen(false)}
        onAppend={(entry) => projects.append(entry)}
      />

      <Accordion
        title={t('build.extras.projects.title')}
        content={
          <div className="space-y-3">
            <Button
              type="button"
              variant="ghost"
              className="gap-1.5"
              onClick={() => setGithubOpen(true)}
            >
              <GitBranch size={14} />
              {t('build.extras.projects.github.trigger')}
            </Button>
            <FieldArrayList
              fields={projects.fields}
              onAppend={() => projects.append({ name: '', description: '', link: '' })}
              onRemove={projects.remove}
              addLabel={t('build.extras.projects.add')}
              removeLabel={t('build.remove')}
              emptyLabel={t('build.extras.projects.empty')}
              icon={FolderGit2}
              render={(index) => (
                <div className="space-y-2.5">
                  <WizardField label={t('build.extras.projects.name')}>
                    <Controller
                      control={control}
                      name={`projects.${index}.name`}
                      render={({ field }) => (
                        <Input
                          className="w-full"
                          value={field.value ?? ''}
                          onChange={field.onChange}
                          onBlur={field.onBlur}
                          placeholder={t('build.extras.projects.namePlaceholder')}
                        />
                      )}
                    />
                  </WizardField>
                  <WizardField label={t('build.extras.projects.description')}>
                    <Controller
                      control={control}
                      name={`projects.${index}.description`}
                      render={({ field }) => (
                        <TextArea
                          variant="glass"
                          value={field.value ?? ''}
                          onChange={field.onChange}
                          onBlur={field.onBlur}
                          rows={2}
                          placeholder={t('build.extras.projects.descriptionPlaceholder')}
                        />
                      )}
                    />
                  </WizardField>
                  <WizardField
                    label={t('build.extras.link')}
                    hint={t('build.extras.linkHint')}
                    error={msg(errors.projects?.[index]?.link?.message)}
                  >
                    <Controller
                      control={control}
                      name={`projects.${index}.link`}
                      render={({ field }) => (
                        <Input
                          className="w-full"
                          value={field.value ?? ''}
                          onChange={field.onChange}
                          onBlur={field.onBlur}
                          placeholder={t('build.extras.linkPlaceholder')}
                        />
                      )}
                    />
                  </WizardField>
                </div>
              )}
            />
          </div>
        }
      />

      <Accordion
        title={t('build.extras.publications.title')}
        content={
          <FieldArrayList
            fields={publications.fields}
            onAppend={() => publications.append({ title: '', venue: '', year: '', link: '' })}
            onRemove={publications.remove}
            addLabel={t('build.extras.publications.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.publications.empty')}
            icon={BookText}
            render={(index) => (
              <div className="space-y-2.5">
                <WizardField label={t('build.extras.publications.titleField')}>
                  <Controller
                    control={control}
                    name={`publications.${index}.title`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.publications.titlePlaceholder')}
                      />
                    )}
                  />
                </WizardField>
                <div className="grid grid-cols-1 gap-2.5 @xs:grid-cols-2">
                  <WizardField label={t('build.extras.publications.venue')}>
                    <Controller
                      control={control}
                      name={`publications.${index}.venue`}
                      render={({ field }) => (
                        <Input
                          className="w-full"
                          value={field.value ?? ''}
                          onChange={field.onChange}
                          onBlur={field.onBlur}
                          placeholder={t('build.extras.publications.venuePlaceholder')}
                        />
                      )}
                    />
                  </WizardField>
                  <WizardField
                    label={t('build.extras.publications.year')}
                    error={msg(errors.publications?.[index]?.year?.message)}
                  >
                    <Controller
                      control={control}
                      name={`publications.${index}.year`}
                      render={({ field }) => (
                        <Input
                          className="w-full"
                          value={field.value ?? ''}
                          onChange={field.onChange}
                          onBlur={field.onBlur}
                          placeholder={t('build.extras.yearPlaceholder')}
                        />
                      )}
                    />
                  </WizardField>
                </div>
                <WizardField
                  label={t('build.extras.link')}
                  hint={t('build.extras.linkHint')}
                  error={msg(errors.publications?.[index]?.link?.message)}
                >
                  <Controller
                    control={control}
                    name={`publications.${index}.link`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.linkPlaceholder')}
                      />
                    )}
                  />
                </WizardField>
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.awards.title')}
        content={
          <FieldArrayList
            fields={awards.fields}
            onAppend={() => awards.append({ title: '', detail: '', year: '' })}
            onRemove={awards.remove}
            addLabel={t('build.extras.awards.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.awards.empty')}
            icon={Award}
            render={(index) => (
              <div className="grid grid-cols-[1fr_1fr_5rem] gap-2.5">
                <WizardField label={t('build.extras.entryTitle')}>
                  <Controller
                    control={control}
                    name={`awards.${index}.title`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.awards.titlePlaceholder')}
                      />
                    )}
                  />
                </WizardField>
                <WizardField label={t('build.extras.entryDetail')}>
                  <Controller
                    control={control}
                    name={`awards.${index}.detail`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.awards.detailPlaceholder')}
                      />
                    )}
                  />
                </WizardField>
                <WizardField
                  label={t('build.extras.entryYear')}
                  error={msg(errors.awards?.[index]?.year?.message)}
                >
                  <Controller
                    control={control}
                    name={`awards.${index}.year`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.yearPlaceholder')}
                      />
                    )}
                  />
                </WizardField>
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.volunteer.title')}
        content={
          <FieldArrayList
            fields={volunteer.fields}
            onAppend={() => volunteer.append({ title: '', detail: '', year: '' })}
            onRemove={volunteer.remove}
            addLabel={t('build.extras.volunteer.add')}
            removeLabel={t('build.remove')}
            emptyLabel={t('build.extras.volunteer.empty')}
            icon={HeartHandshake}
            render={(index) => (
              <div className="grid grid-cols-[1fr_1fr_5rem] gap-2.5">
                <WizardField label={t('build.extras.entryTitle')}>
                  <Controller
                    control={control}
                    name={`volunteer.${index}.title`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.volunteer.titlePlaceholder')}
                      />
                    )}
                  />
                </WizardField>
                <WizardField label={t('build.extras.entryDetail')}>
                  <Controller
                    control={control}
                    name={`volunteer.${index}.detail`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.volunteer.detailPlaceholder')}
                      />
                    )}
                  />
                </WizardField>
                <WizardField
                  label={t('build.extras.entryYear')}
                  error={msg(errors.volunteer?.[index]?.year?.message)}
                >
                  <Controller
                    control={control}
                    name={`volunteer.${index}.year`}
                    render={({ field }) => (
                      <Input
                        className="w-full"
                        value={field.value ?? ''}
                        onChange={field.onChange}
                        onBlur={field.onBlur}
                        placeholder={t('build.extras.yearPlaceholder')}
                      />
                    )}
                  />
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
            <Controller
              control={control}
              name="languages"
              render={({ field }) => (
                <TextArea
                  variant="glass"
                  value={(field.value ?? []).join('\n')}
                  onChange={(e) => field.onChange(e.target.value.split('\n'))}
                  onBlur={field.onBlur}
                  rows={3}
                  placeholder={t('build.extras.languages.placeholder')}
                />
              )}
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
            <Controller
              control={control}
              name="certifications"
              render={({ field }) => (
                <TextArea
                  variant="glass"
                  value={(field.value ?? []).join('\n')}
                  onChange={(e) => field.onChange(e.target.value.split('\n'))}
                  onBlur={field.onBlur}
                  rows={3}
                  placeholder={t('build.extras.certifications.placeholder')}
                />
              )}
            />
          </WizardField>
        }
      />
    </div>
  );
}
