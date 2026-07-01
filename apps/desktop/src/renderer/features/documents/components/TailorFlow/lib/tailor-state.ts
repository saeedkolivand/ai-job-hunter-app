import type { TailorTarget } from '../useTailorGeneration';

/**
 * The page-local tailoring wizard form. Model is global (selected via the shared
 * ModelSelector + persisted by the model store), so it is NOT a field here.
 */
export interface TailorWizardState {
  resume: string;
  outputType: TailorTarget; // 'resume' | 'cover' | 'both'
  researchCompany: boolean;
}

/** Seed defaults for a fresh job — resume pre-filled from the autopilot's base. */
export function buildTailorDefaults(resumeText?: string): TailorWizardState {
  return { resume: resumeText ?? '', outputType: 'both', researchCompany: false };
}
