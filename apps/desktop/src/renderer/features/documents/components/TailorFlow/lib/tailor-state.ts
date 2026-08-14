import type { TailorTarget } from '../lib/tailor-target';

/**
 * The page-local tailoring wizard form. Model is global (selected via the shared
 * ModelSelector + persisted by the model store), so it is NOT a field here.
 */
export interface TailorWizardState {
  resume: string;
  outputType: TailorTarget; // 'resume' | 'cover' | 'both'
  researchCompany: boolean;
  /** Which saved résumé backs `resume`, unedited — see `tailor-schema`'s doc. */
  resumeDocId?: string;
}

/**
 * Seed defaults for a fresh job — resume pre-filled from the autopilot's base.
 *
 * `researchCompany` is capability-driven: the caller passes the active model's
 * `supportsWebSearch` so the "search company" toggle defaults ON for a model
 * that can web-search and OFF otherwise. Defaults to `false` (the safe fallback
 * used while the capability is still resolving, or when it can't).
 */
export function buildTailorDefaults(
  resumeText?: string,
  researchCompany = false
): TailorWizardState {
  return { resume: resumeText ?? '', outputType: 'both', researchCompany };
}
