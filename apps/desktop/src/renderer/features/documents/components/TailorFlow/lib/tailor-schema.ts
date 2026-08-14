import { z } from 'zod';

/**
 * Validation for the tailoring wizard. Error messages are i18n KEYS — resolved
 * in the component via `t(...)`, matching the CreationWizard convention.
 */
export const tailorWizardSchema = z.object({
  resume: z.string().trim().min(1, 'autopilot.apply.wizard.validation.resumeRequired'),
  outputType: z.enum(['resume', 'cover', 'both']),
  researchCompany: z.boolean(),
  // Which saved résumé (if any) backs `resume`, unedited since it loaded —
  // optional/untyped-by-zod on purpose: it never gates "Next"/"Generate", it
  // just rides along so `useTailorPipeline` can send `resumeId` instead of
  // `resumeText`. Cleared (by ResumeInputCard's `onDocIdChange`) the moment
  // the text is hand-edited.
  resumeDocId: z.string().optional(),
});
