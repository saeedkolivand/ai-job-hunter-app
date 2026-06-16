import {
  Children,
  cloneElement,
  type FormHTMLAttributes,
  isValidElement,
  type ReactElement,
  type ReactNode,
  useId,
} from 'react';
import {
  Controller,
  type FieldValues,
  FormProvider,
  type RegisterOptions,
  type SubmitHandler,
  useFormContext,
  type UseFormReturn,
} from 'react-hook-form';

import { cn } from '../../lib/cn';

// ─── Form ─────────────────────────────────────────────────────────────────────

export interface FormProps<T extends FieldValues> extends Omit<
  FormHTMLAttributes<HTMLFormElement>,
  'onSubmit'
> {
  /** The instance from `useForm()` (re-exported from `@ajh/ui`). */
  form: UseFormReturn<T>;
  /** Called with validated values on submit (wraps `handleSubmit`). */
  onSubmit?: SubmitHandler<T>;
  children: ReactNode;
}

/**
 * Thin wrapper over react-hook-form's `FormProvider` + a native `<form>`. Provides
 * the form context so {@link FormField} children bind by `name` — features import
 * `Form`, `FormField`, and `useForm` from `@ajh/ui` and never touch react-hook-form
 * directly. RHF stays the engine (validation, state); this is only the boundary.
 */
export function Form<T extends FieldValues>({
  form,
  onSubmit,
  children,
  className,
  ...rest
}: FormProps<T>) {
  return (
    <FormProvider {...form}>
      <form
        noValidate
        onSubmit={onSubmit ? form.handleSubmit(onSubmit) : undefined}
        className={className}
        {...rest}
      >
        {children}
      </form>
    </FormProvider>
  );
}

// ─── FormField ──────────────────────────────────────────────────────────────

export interface FormFieldProps {
  /** Field path in the form values. */
  name: string;
  /** Label text rendered above the control. */
  label?: ReactNode;
  /** Small hint beside the label. */
  hint?: ReactNode;
  /** Marks the field required (adds a red asterisk; pass `rules` for validation). */
  required?: boolean;
  /** react-hook-form validation rules. */
  rules?: Omit<RegisterOptions, 'valueAsNumber' | 'valueAsDate' | 'setValueAs' | 'disabled'>;
  /**
   * The single value-based control (`Input`, `TextArea`, `SelectDropdown`, …). It
   * receives `value` / `onChange` / `onBlur` / `name` / `id` / `aria-invalid` from
   * the bound field. (For non-value controls like Switch, bind via `Controller`.)
   */
  children: ReactElement;
  className?: string;
}

/**
 * A labelled, validation-aware field bound to the enclosing {@link Form} by `name`.
 * Wires the control to react-hook-form (value + onChange + onBlur), associates a
 * `<label>`, and renders the inline error — superseding the per-feature
 * `Controller` + label/error boilerplate.
 */
export function FormField({
  name,
  label,
  hint,
  required,
  rules,
  children,
  className,
}: FormFieldProps) {
  const { control } = useFormContext();
  const generatedId = useId();
  const child = Children.only(children);
  const childId =
    isValidElement(child) && typeof (child.props as { id?: string }).id === 'string'
      ? (child.props as { id: string }).id
      : undefined;
  const fieldId = childId ?? generatedId;

  return (
    <Controller
      name={name}
      control={control}
      rules={rules}
      render={({ field, fieldState }) => (
        <div className={cn('space-y-1.5', className)}>
          {label != null && (
            <label
              htmlFor={fieldId}
              className="flex items-center gap-1 text-xs font-medium text-foreground/60"
            >
              {label}
              {required && <span className="text-red-400">*</span>}
            </label>
          )}
          {hint != null && <span className="text-[10px] text-foreground/40">{hint}</span>}
          {isValidElement(child)
            ? cloneElement(child as ReactElement<Record<string, unknown>>, {
                id: fieldId,
                name: field.name,
                value: field.value ?? '',
                onChange: field.onChange,
                onBlur: field.onBlur,
                'aria-invalid': fieldState.error ? true : undefined,
              })
            : child}
          {fieldState.error?.message != null && (
            <p className="text-[10px] text-red-400/80">{fieldState.error.message}</p>
          )}
        </div>
      )}
    />
  );
}
