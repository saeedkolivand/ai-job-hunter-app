/**
 * Pure read model for the per-stage overrides UI: which model each pipeline
 * stage will actually run on, and whether that answer is broken.
 *
 * Kept free of React/queries so both states that matter — "this stage is
 * overridden" and "this override cannot run" — are unit-testable.
 */

import { type AiStageOverride, PIPELINE_STAGES, type PipelineStage } from '@ajh/shared';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

/**
 * Stages that make NO provider call: `assemble` is a pure renderer and
 * `validate` is deterministic. The backend rejects an override for them, and
 * offering a control that is refused on save is worse than offering none — so
 * the vocabulary is filtered here rather than in each consumer.
 *
 * Derived from {@link PIPELINE_STAGES} (never a second hand-written list), so
 * a stage added upstream shows up here automatically instead of silently
 * missing from Settings.
 */
export const FREE_PIPELINE_STAGES = [
  'assemble',
  'validate',
] as const satisfies readonly PipelineStage[];

/** The stages a model override can actually change. */
export const OVERRIDABLE_PIPELINE_STAGES: readonly PipelineStage[] = PIPELINE_STAGES.filter(
  (stage) => !(FREE_PIPELINE_STAGES as readonly string[]).includes(stage)
);

/**
 * Why a stage cannot run as configured. Surfaced BEFORE a run because the
 * backend refuses at run time rather than falling back to the active provider —
 * an unconfigured override is a failed run, not a silent downgrade.
 */
export type StageRoutingProblem =
  /** No provider at all, or one with no key/binary/server behind it. */
  | 'unconfigured'
  /** A configured provider that needs a model and has none. */
  | 'no-model';

export interface StageRouting {
  stage: PipelineStage;
  /** Set only when the user explicitly pinned this stage. Absent = the active
   *  provider runs it, which is NOT the same as an override equal to it. */
  override?: AiStageOverride;
  provider?: string;
  /** Empty/absent is legitimate for a CLI agent — it runs its own default. */
  model?: string;
  problem?: StageRoutingProblem;
}

export interface StageRoutingInput {
  overrides: Record<string, AiStageOverride>;
  /** The active provider row — what an un-overridden stage resolves through. */
  active: { provider?: string; model?: string };
  /** Live "can this provider be reached at all" answer, from the same key/health
   *  status the provider rows use. */
  isConfigured: (provider: string) => boolean;
  /** Defaults to {@link OVERRIDABLE_PIPELINE_STAGES}. */
  stages?: readonly PipelineStage[];
}

/** A CLI agent runs whatever model its own login is configured for, so an empty
 *  model is a valid override there and a missing one everywhere else. */
export function providerNeedsModel(provider: string): boolean {
  return PROVIDERS[provider as AiProvider]?.kind !== 'cli-agent';
}

/** One row per overridable stage, in pipeline order. */
export function resolveStageRouting(input: StageRoutingInput): StageRouting[] {
  const { overrides, active, isConfigured, stages = OVERRIDABLE_PIPELINE_STAGES } = input;

  return stages.map((stage) => {
    const override = overrides[stage];
    const provider = override?.provider ?? active.provider;
    const model = override?.model ?? active.model;

    const problem: StageRoutingProblem | undefined = !provider
      ? 'unconfigured'
      : !isConfigured(provider)
        ? 'unconfigured'
        : providerNeedsModel(provider) && !model
          ? 'no-model'
          : undefined;

    return { stage, override, provider, model, problem };
  });
}
