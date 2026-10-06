import { Award, BookText, FolderGit2, GitBranch, HeartHandshake } from 'lucide-react';
import { useState } from 'react';
import { useFieldArray, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Accordion, Button } from '@ajh/ui';

import type { BuilderFormValues } from '../../../types';
import { FieldArrayList } from '../../FieldArrayList';
import { GitHubImportModal } from '../../GitHubImportModal';
import { ExtrasLinesField, ExtrasTextField } from './ExtrasFields';

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
              onAppend={() =>
                projects.append({ name: '', description: '', link: '', technologies: '' })
              }
              onRemove={projects.remove}
              addLabel={t('build.extras.projects.add')}
              removeLabel={t('build.remove')}
              emptyLabel={t('build.extras.projects.empty')}
              icon={FolderGit2}
              render={(index) => (
                <div className="space-y-2.5">
                  <ExtrasTextField
                    name={`projects.${index}.name`}
                    label={t('build.extras.projects.name')}
                    placeholder={t('build.extras.projects.namePlaceholder')}
                  />
                  <ExtrasTextField
                    multiline
                    name={`projects.${index}.description`}
                    label={t('build.extras.projects.description')}
                    placeholder={t('build.extras.projects.descriptionPlaceholder')}
                  />
                  <ExtrasTextField
                    name={`projects.${index}.technologies`}
                    label={t('build.extras.projects.technologies')}
                    hint={t('build.extras.projects.technologiesHint')}
                    placeholder={t('build.extras.projects.technologiesPlaceholder')}
                  />
                  <ExtrasTextField
                    name={`projects.${index}.link`}
                    label={t('build.extras.link')}
                    hint={t('build.extras.linkHint')}
                    placeholder={t('build.extras.linkPlaceholder')}
                    error={msg(errors.projects?.[index]?.link?.message)}
                  />
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
                <ExtrasTextField
                  name={`publications.${index}.title`}
                  label={t('build.extras.publications.titleField')}
                  placeholder={t('build.extras.publications.titlePlaceholder')}
                />
                <div className="grid grid-cols-1 gap-2.5 @xs:grid-cols-2">
                  <ExtrasTextField
                    name={`publications.${index}.venue`}
                    label={t('build.extras.publications.venue')}
                    placeholder={t('build.extras.publications.venuePlaceholder')}
                  />
                  <ExtrasTextField
                    name={`publications.${index}.year`}
                    label={t('build.extras.publications.year')}
                    placeholder={t('build.extras.yearPlaceholder')}
                    error={msg(errors.publications?.[index]?.year?.message)}
                  />
                </div>
                <ExtrasTextField
                  name={`publications.${index}.link`}
                  label={t('build.extras.link')}
                  hint={t('build.extras.linkHint')}
                  placeholder={t('build.extras.linkPlaceholder')}
                  error={msg(errors.publications?.[index]?.link?.message)}
                />
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
                <ExtrasTextField
                  name={`awards.${index}.title`}
                  label={t('build.extras.entryTitle')}
                  placeholder={t('build.extras.awards.titlePlaceholder')}
                />
                <ExtrasTextField
                  name={`awards.${index}.detail`}
                  label={t('build.extras.entryDetail')}
                  placeholder={t('build.extras.awards.detailPlaceholder')}
                />
                <ExtrasTextField
                  name={`awards.${index}.year`}
                  label={t('build.extras.entryYear')}
                  placeholder={t('build.extras.yearPlaceholder')}
                  error={msg(errors.awards?.[index]?.year?.message)}
                />
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
                <ExtrasTextField
                  name={`volunteer.${index}.title`}
                  label={t('build.extras.entryTitle')}
                  placeholder={t('build.extras.volunteer.titlePlaceholder')}
                />
                <ExtrasTextField
                  name={`volunteer.${index}.detail`}
                  label={t('build.extras.entryDetail')}
                  placeholder={t('build.extras.volunteer.detailPlaceholder')}
                />
                <ExtrasTextField
                  name={`volunteer.${index}.year`}
                  label={t('build.extras.entryYear')}
                  placeholder={t('build.extras.yearPlaceholder')}
                  error={msg(errors.volunteer?.[index]?.year?.message)}
                />
              </div>
            )}
          />
        }
      />

      <Accordion
        title={t('build.extras.languages.title')}
        content={
          <ExtrasLinesField
            name="languages"
            label={t('build.extras.languages.label')}
            hint={t('build.extras.languages.hint')}
            placeholder={t('build.extras.languages.placeholder')}
          />
        }
      />

      <Accordion
        title={t('build.extras.certifications.title')}
        content={
          <ExtrasLinesField
            name="certifications"
            label={t('build.extras.certifications.label')}
            hint={t('build.extras.certifications.hint')}
            placeholder={t('build.extras.certifications.placeholder')}
          />
        }
      />
    </div>
  );
}
