import { type Control, Controller, type FieldError } from 'react-hook-form';

import { useTranslation } from '@ajh/translations';
import { Input } from '@ajh/ui';

import type { ContactFormValues } from './contactSchema';

export const FIELD_CLASS = 'flex flex-col gap-1.5';
export const LABEL_CLASS = 'text-xs font-medium text-foreground/70';

interface ContactTextFieldProps {
  control: Control<ContactFormValues>;
  name: 'fullName' | 'email' | 'phone' | 'linkedin' | 'github' | 'website';
  id: string;
  label: string;
  placeholder?: string;
  type?: 'email';
  /** Inline validation hint (an i18n key) — never gates persistence. */
  error?: FieldError;
  /** Called on blur, after the field's own blur handler. */
  onCommit: () => void;
}

/** One labelled text input bound to the isolated contact-profile form; persists on blur. */
export function ContactTextField({
  control,
  name,
  id,
  label,
  placeholder,
  type,
  error,
  onCommit,
}: ContactTextFieldProps) {
  const { t } = useTranslation();
  return (
    <div className={FIELD_CLASS}>
      <label className={LABEL_CLASS} htmlFor={id}>
        {label}
      </label>
      <Controller
        control={control}
        name={name}
        render={({ field }) => (
          <Input
            id={id}
            type={type}
            value={field.value}
            onChange={field.onChange}
            onBlur={() => {
              field.onBlur();
              onCommit();
            }}
            placeholder={placeholder}
            aria-invalid={error ? true : undefined}
          />
        )}
      />
      {error && <p className="text-xs text-amber-400/80">{t(error.message ?? '')}</p>}
    </div>
  );
}
