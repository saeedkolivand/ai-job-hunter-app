export { Form, FormField, type FormFieldProps, type FormProps } from './Form';

// Re-export the react-hook-form surface through @ajh/ui so feature code imports
// everything form-related from one place and never depends on react-hook-form
// directly (enforced by ESLint). RHF remains the engine behind Form/FormField.
export {
  type Control,
  Controller,
  type FieldErrors,
  type FieldValues,
  type Path,
  type RegisterOptions,
  type SubmitHandler,
  useFieldArray,
  type UseFieldArrayReturn,
  useForm,
  useFormContext,
  type UseFormReturn,
  useWatch,
} from 'react-hook-form';
