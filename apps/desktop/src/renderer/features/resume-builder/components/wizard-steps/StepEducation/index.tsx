import { GraduationCap } from 'lucide-react';
import { Controller, useFieldArray, useFormContext } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Input, LocationInput, TextArea } from '@ajh/ui';

import { useAppClient } from '@/providers/AppClientProvider';

import type { BuilderFormValues } from '../../../types';
import { FieldArrayList } from '../../FieldArrayList';
import { MonthYearField } from '../../MonthYearField';
import { WizardField } from '../../WizardField';

const blank = () => ({
  degree: '',
  institution: '',
  location: '',
  startDate: '',
  endDate: '',
  details: '',
});

/** Repeatable education entries (core section). */
export function StepEducation() {
  const { t } = useTranslation();
  const api = useAppClient();
  const { control, formState } = useFormContext<BuilderFormValues>();
  const { fields, append, remove } = useFieldArray({ control, name: 'education' });

  return (
    <FieldArrayList
      fields={fields}
      onAppend={() => append(blank())}
      onRemove={remove}
      addLabel={t('build.education.add')}
      removeLabel={t('build.remove')}
      emptyLabel={t('build.education.empty')}
      emptyDescription={t('build.education.emptyDescription')}
      icon={GraduationCap}
      render={(index) => {
        const identityError = formState.errors.education?.[index]?.degree?.message;
        return (
          <div className="space-y-2.5">
            <div className="grid grid-cols-1 gap-2.5 @xs:grid-cols-2">
              <WizardField
                label={t('build.education.degree')}
                error={identityError ? t(identityError) : undefined}
              >
                <Controller
                  control={control}
                  name={`education.${index}.degree`}
                  render={({ field }) => (
                    <Input
                      className="w-full"
                      value={field.value ?? ''}
                      onChange={field.onChange}
                      onBlur={field.onBlur}
                      placeholder={t('build.education.degreePlaceholder')}
                    />
                  )}
                />
              </WizardField>
              <WizardField label={t('build.education.institution')}>
                <Controller
                  control={control}
                  name={`education.${index}.institution`}
                  render={({ field }) => (
                    <Input
                      className="w-full"
                      value={field.value ?? ''}
                      onChange={field.onChange}
                      onBlur={field.onBlur}
                      placeholder={t('build.education.institutionPlaceholder')}
                    />
                  )}
                />
              </WizardField>
            </div>

            <div className="grid grid-cols-1 gap-2.5 @sm:grid-cols-3">
              <WizardField label={t('build.education.location')}>
                <Controller
                  control={control}
                  name={`education.${index}.location`}
                  render={({ field }) => (
                    <LocationInput
                      value={field.value ?? ''}
                      onChange={field.onChange}
                      placeholder={t('build.education.locationPlaceholder')}
                      onFetchSuggestions={(q) => api.geocode.suggest(q)}
                    />
                  )}
                />
              </WizardField>
              <WizardField label={t('build.education.start')}>
                <Controller
                  control={control}
                  name={`education.${index}.startDate`}
                  render={({ field }) => (
                    <MonthYearField value={field.value ?? ''} onChange={field.onChange} />
                  )}
                />
              </WizardField>
              <WizardField label={t('build.education.end')}>
                <Controller
                  control={control}
                  name={`education.${index}.endDate`}
                  render={({ field }) => (
                    <MonthYearField value={field.value ?? ''} onChange={field.onChange} />
                  )}
                />
              </WizardField>
            </div>

            <WizardField
              label={t('build.education.details')}
              hint={t('build.education.detailsHint')}
            >
              <Controller
                control={control}
                name={`education.${index}.details`}
                render={({ field }) => (
                  <TextArea
                    variant="glass"
                    value={field.value ?? ''}
                    onChange={field.onChange}
                    onBlur={field.onBlur}
                    rows={2}
                  />
                )}
              />
            </WizardField>
          </div>
        );
      }}
    />
  );
}
