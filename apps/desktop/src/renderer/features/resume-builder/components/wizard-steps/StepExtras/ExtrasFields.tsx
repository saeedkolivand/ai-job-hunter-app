import { Controller, useFormContext } from 'react-hook-form';

import { Input, TextArea } from '@ajh/ui';

import type { BuilderFormValues } from '../../../types';
import { WizardField } from '../../WizardField';

/** Every plain-string field of the repeatable extras sections (`<section>.<index>.<key>`). */
type ExtrasTextPath =
  | `projects.${number}.${'name' | 'description' | 'technologies' | 'link'}`
  | `publications.${number}.${'title' | 'venue' | 'year' | 'link'}`
  | `${'awards' | 'volunteer'}.${number}.${'title' | 'detail' | 'year'}`;

interface ExtrasTextFieldProps {
  name: ExtrasTextPath;
  label: string;
  placeholder: string;
  hint?: string;
  /** Already-translated validation message. */
  error?: string;
  /** Render a glass `TextArea` (2 rows) instead of a single-line `Input`. */
  multiline?: boolean;
}

/** A labelled single-value field bound to the surrounding builder form. */
export function ExtrasTextField({
  name,
  label,
  placeholder,
  hint,
  error,
  multiline,
}: ExtrasTextFieldProps) {
  const { control } = useFormContext<BuilderFormValues>();
  return (
    <WizardField label={label} hint={hint} error={error}>
      <Controller
        control={control}
        name={name}
        render={({ field }) =>
          multiline ? (
            <TextArea
              variant="glass"
              value={field.value ?? ''}
              onChange={field.onChange}
              onBlur={field.onBlur}
              rows={2}
              placeholder={placeholder}
            />
          ) : (
            <Input
              className="w-full"
              value={field.value ?? ''}
              onChange={field.onChange}
              onBlur={field.onBlur}
              placeholder={placeholder}
            />
          )
        }
      />
    </WizardField>
  );
}

interface ExtrasLinesFieldProps {
  name: 'languages' | 'certifications';
  label: string;
  hint: string;
  placeholder: string;
}

/** A one-entry-per-line textarea bound to a string-array form field. */
export function ExtrasLinesField({ name, label, hint, placeholder }: ExtrasLinesFieldProps) {
  const { control } = useFormContext<BuilderFormValues>();
  return (
    <WizardField label={label} hint={hint}>
      <Controller
        control={control}
        name={name}
        render={({ field }) => (
          <TextArea
            variant="glass"
            value={(field.value ?? []).join('\n')}
            onChange={(e) => field.onChange(e.target.value.split('\n'))}
            onBlur={field.onBlur}
            rows={3}
            placeholder={placeholder}
          />
        )}
      />
    </WizardField>
  );
}
