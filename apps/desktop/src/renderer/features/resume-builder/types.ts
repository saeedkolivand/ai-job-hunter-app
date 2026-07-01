import type { BuilderForm } from './lib/schema';

/**
 * Re-export the form value shape under a builder-local name. Wizard steps read
 * and write these fields through react-hook-form (`useFormContext` /
 * `Controller` / `useFieldArray`) — no per-step props are needed; the live
 * editing state lives in the form, synced one-way into the `resumeBuilder`
 * session slice by {@link BuilderWizard}.
 */
export type BuilderFormValues = BuilderForm;
