import type { ComponentType } from 'react';

import { ContextFitStep } from './ContextFitStep';
import { InstalledModelsStep } from './InstalledModelsStep';
import { RecommendationsStep } from './RecommendationsStep';
import { ThinkingRiskStep } from './ThinkingRiskStep';
import type { AdvisorStepProps } from './types';

/**
 * The advisor's steps, in order — the flat `{ id, component }` list
 * `ONBOARDING_STEPS` uses, so the shell stays a loop over an array rather than
 * a switch that has to be edited in two places to add a step.
 *
 * `id` doubles as the i18n key segment for the step's title.
 */
export const ADVISOR_STEPS = [
  { id: 'models', component: InstalledModelsStep },
  { id: 'fit', component: ContextFitStep },
  { id: 'recommend', component: RecommendationsStep },
  { id: 'risk', component: ThinkingRiskStep },
] as const satisfies ReadonlyArray<{ id: string; component: ComponentType<AdvisorStepProps> }>;

/** The step ids, kept as literals so an i18n key built from one is checkable. */
export type AdvisorStepId = (typeof ADVISOR_STEPS)[number]['id'];
