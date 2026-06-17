import { Briefcase } from 'lucide-react';
import { type Control, Controller, useFieldArray, useFormContext, useWatch } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Input, LocationInput, TextArea } from '@ajh/ui';

import { useAppClient } from '@/providers/AppClientProvider';

import type { BuilderFormValues } from '../../../types';
import { FieldArrayList } from '../../FieldArrayList';
import { MonthYearField } from '../../MonthYearField';
import { WizardField } from '../../WizardField';

const blank = () => ({
  title: '',
  company: '',
  location: '',
  startDate: '',
  endDate: '',
  current: false,
  bullets: [],
});

/**
 * One experience entry. Extracted so the End-date `present` flag can read just
 * `experience.${index}.current` via `useWatch` — reading it with the form-level
 * `watch(...)` inside the render prop would subscribe the whole list and re-render
 * every entry on any keystroke.
 */
function ExperienceEntry({
  index,
  control,
  identityError,
}: {
  index: number;
  control: Control<BuilderFormValues>;
  identityError?: string;
}) {
  const { t } = useTranslation();
  const api = useAppClient();
  const present = useWatch({ control, name: `experience.${index}.current` });

  return (
    <div className="space-y-2.5">
      <div className="grid grid-cols-1 gap-2.5 @xs:grid-cols-2">
        <WizardField
          label={t('build.experience.title')}
          error={identityError ? t(identityError) : undefined}
        >
          <Controller
            control={control}
            name={`experience.${index}.title`}
            render={({ field }) => (
              <Input
                className="w-full"
                value={field.value ?? ''}
                onChange={field.onChange}
                onBlur={field.onBlur}
                placeholder={t('build.experience.titlePlaceholder')}
              />
            )}
          />
        </WizardField>
        <WizardField label={t('build.experience.company')}>
          <Controller
            control={control}
            name={`experience.${index}.company`}
            render={({ field }) => (
              <Input
                className="w-full"
                value={field.value ?? ''}
                onChange={field.onChange}
                onBlur={field.onBlur}
                placeholder={t('build.experience.companyPlaceholder')}
              />
            )}
          />
        </WizardField>
      </div>

      <div className="grid grid-cols-1 gap-2.5 @sm:grid-cols-3">
        <WizardField label={t('build.experience.location')}>
          <Controller
            control={control}
            name={`experience.${index}.location`}
            render={({ field }) => (
              <LocationInput
                value={field.value ?? ''}
                onChange={field.onChange}
                placeholder={t('build.experience.locationPlaceholder')}
                onFetchSuggestions={(q) => api.geocode.suggest(q)}
              />
            )}
          />
        </WizardField>
        <WizardField label={t('build.experience.start')}>
          <Controller
            control={control}
            name={`experience.${index}.startDate`}
            render={({ field }) => (
              <MonthYearField value={field.value ?? ''} onChange={field.onChange} />
            )}
          />
        </WizardField>
        <WizardField label={t('build.experience.end')}>
          <Controller
            control={control}
            name={`experience.${index}.endDate`}
            render={({ field }) => (
              <MonthYearField
                value={field.value ?? ''}
                onChange={field.onChange}
                present={present}
              />
            )}
          />
        </WizardField>
      </div>

      <Controller
        control={control}
        name={`experience.${index}.current`}
        render={({ field }) => (
          <label className="flex items-center gap-2 text-xs text-foreground/60">
            <input
              type="checkbox"
              checked={field.value ?? false}
              onChange={(e) => field.onChange(e.target.checked)}
              className="accent-brand"
            />
            {t('build.experience.current')}
          </label>
        )}
      />

      <WizardField label={t('build.experience.bullets')} hint={t('build.experience.bulletsHint')}>
        <Controller
          control={control}
          name={`experience.${index}.bullets`}
          render={({ field }) => (
            <TextArea
              variant="glass"
              value={(field.value ?? []).join('\n')}
              onChange={(e) => field.onChange(e.target.value.split('\n'))}
              onBlur={field.onBlur}
              rows={4}
              placeholder={t('build.experience.bulletsPlaceholder')}
            />
          )}
        />
      </WizardField>
    </div>
  );
}

/** Repeatable work-experience entries (core section). */
export function StepExperience() {
  const { t } = useTranslation();
  const { control, formState } = useFormContext<BuilderFormValues>();
  const { fields, append, remove } = useFieldArray({ control, name: 'experience' });

  return (
    <FieldArrayList
      fields={fields}
      onAppend={() => append(blank())}
      onRemove={remove}
      addLabel={t('build.experience.add')}
      removeLabel={t('build.remove')}
      emptyLabel={t('build.experience.empty')}
      emptyDescription={t('build.experience.emptyDescription')}
      icon={Briefcase}
      render={(index) => (
        <ExperienceEntry
          index={index}
          control={control}
          identityError={formState.errors.experience?.[index]?.title?.message}
        />
      )}
    />
  );
}
