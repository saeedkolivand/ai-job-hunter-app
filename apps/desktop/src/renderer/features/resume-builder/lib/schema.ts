import { z } from 'zod';

import type { InterviewAnswers } from '@/lib/generate';

/**
 * Zod schema for the Resume Builder form — the single source of truth for the
 * in-wizard editing layer (react-hook-form + `zodResolver`). It mirrors exactly
 * the {@link InterviewAnswers} fields the wizard edits; contact identity and the
 * generation options (language / template / ATS) live outside the form.
 *
 * Every field is a plain (non-optional) string / array: the wizard always seeds
 * complete `defaultValues` from the persisted answers, so the schema's input and
 * output types match (a zod-`.default()` input/output mismatch otherwise breaks
 * the `useForm`/`zodResolver` generics). Empty strings are the "unset" value and
 * are treated as blank by the refinements below.
 *
 * Messages are i18n KEYS (e.g. `build.validation.url`); the steps resolve them
 * to localized text through `WizardField`'s `error` prop via `t(...)`.
 *
 * Rules (kept lightweight, non-pedantic — identical to the prior manual
 * validator that this schema replaces):
 * - project / publication `link`: a valid http(s) URL when non-empty.
 * - publication / award / volunteer `year`: a 4-digit number when non-empty.
 * - experience entry: if any field is filled, require `title` OR `company`.
 * - education entry: require `degree` OR `institution` when any field is filled.
 */

const isBlank = (v: string | undefined): boolean => !v || !v.trim();

/** Accepts http(s) URLs only; non-pedantic. */
function isValidUrl(value: string): boolean {
  const v = value.trim();
  try {
    const url = new URL(v);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}

/** A blank string or a valid http(s) URL. */
const urlField = z
  .string()
  .refine((v) => isBlank(v) || isValidUrl(v), { message: 'build.validation.url' });

/** A blank string or a 4-digit year. */
const yearField = z
  .string()
  .refine((v) => isBlank(v) || /^\d{4}$/.test(v.trim()), { message: 'build.validation.year' });

const experienceSchema = z
  .object({
    title: z.string(),
    company: z.string(),
    location: z.string(),
    startDate: z.string(),
    endDate: z.string(),
    current: z.boolean(),
    bullets: z.array(z.string()),
  })
  .superRefine((e, ctx) => {
    const touched =
      !isBlank(e.title) ||
      !isBlank(e.company) ||
      !isBlank(e.location) ||
      !isBlank(e.startDate) ||
      !isBlank(e.endDate) ||
      e.bullets.some((b) => !isBlank(b));
    if (touched && isBlank(e.title) && isBlank(e.company)) {
      ctx.addIssue({
        code: 'custom',
        path: ['title'],
        message: 'build.validation.experienceIdentity',
      });
    }
  });

const educationSchema = z
  .object({
    degree: z.string(),
    institution: z.string(),
    location: z.string(),
    startDate: z.string(),
    endDate: z.string(),
    details: z.string(),
  })
  .superRefine((e, ctx) => {
    const touched =
      !isBlank(e.degree) ||
      !isBlank(e.institution) ||
      !isBlank(e.location) ||
      !isBlank(e.startDate) ||
      !isBlank(e.endDate) ||
      !isBlank(e.details);
    if (touched && isBlank(e.degree) && isBlank(e.institution)) {
      ctx.addIssue({
        code: 'custom',
        path: ['degree'],
        message: 'build.validation.educationIdentity',
      });
    }
  });

const projectSchema = z.object({
  name: z.string(),
  description: z.string(),
  link: urlField,
});

const publicationSchema = z.object({
  title: z.string(),
  venue: z.string(),
  year: yearField,
  link: urlField,
});

const entrySchema = z.object({
  title: z.string(),
  detail: z.string(),
  year: yearField,
});

export const builderSchema = z.object({
  headline: z.string(),
  summary: z.string(),
  experience: z.array(experienceSchema),
  education: z.array(educationSchema),
  skills: z.array(z.string()),
  projects: z.array(projectSchema),
  publications: z.array(publicationSchema),
  awards: z.array(entrySchema),
  volunteer: z.array(entrySchema),
  languages: z.array(z.string()),
  certifications: z.array(z.string()),
});

/**
 * The form value shape. Assignable to the in-scope {@link InterviewAnswers}
 * fields (everything except `fullName`, which the contact profile owns).
 */
export type BuilderForm = z.infer<typeof builderSchema>;

// Compile-time guard: the form must stay assignable to InterviewAnswers minus
// the contact-owned `fullName`. If a field drifts, this errors at typecheck.
type _AssignableToAnswers = BuilderForm extends Omit<InterviewAnswers, 'fullName'> ? true : never;
const _assignable: _AssignableToAnswers = true;
void _assignable;
