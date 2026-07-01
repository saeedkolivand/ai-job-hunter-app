import { z } from 'zod';

/**
 * Validation for the tailoring wizard. Error messages are i18n KEYS — resolved
 * in the component via `t(...)`, matching the CreationWizard convention.
 */
export const tailorWizardSchema = z.object({
  resume: z.string().trim().min(1, 'autopilot.apply.wizard.validation.resumeRequired'),
  outputType: z.enum(['resume', 'cover', 'both']),
  researchCompany: z.boolean(),
});
