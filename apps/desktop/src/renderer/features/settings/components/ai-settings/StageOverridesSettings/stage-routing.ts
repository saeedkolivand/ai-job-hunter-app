/**
 * Pure read model for the per-stage overrides UI: which model each pipeline
 * stage will actually run on, and whether that answer is broken.
 *
 * Kept free of React/queries so both states that matter — "this stage is
 * overridden" and "this override cannot run" — are unit-testable.
 */

import {
  type AiStageOverride,
  PIPELINE_STAGES,
  PIPELINE_STAGES_FREE,
  type PipelineStage,
} from '@ajh/shared';

import { PROVIDERS } from '@/lib/ai-providers/provider-meta';
import type { AiProvider } from '@/store/preferences-schema';

/**
 * The stages a model override can actually change: the generated vocabulary
 * minus the ones that make no provider call.
 *
 * BOTH lists come from `@ajh/shared` — the stage names are codegen'd into Rust
 * and the free set is pinned against `Pipeline::free_stage_names()` of both
 * depths, so neither is hand-copied here. The backend REFUSES an override on a
 * free stage, and offering a control that is rejected on save is worse than
 * offering none.
 */
export const OVERRIDABLE_PIPELINE_STAGES: readonly PipelineStage[] = PIPELINE_STAGES.filter(
  (stage) => !(PIPELINE_STAGES_FREE as readonly string[]).includes(stage)
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
  /**
   * A problem with THIS ROW'S OWN override — never one inherited from the
   * active provider.
   *
   * A broken active provider is one problem, not seven: repeating it per row
   * both buries the rows that really are misconfigured and offers advice ("put
   * this stage back on the default") that is meaningless on a row already on
   * the default. See {@link resolveActiveProblem} for that half.
   */
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

/** Shared predicate: what is wrong with running on this provider + model. */
function routingProblem(
  provider: string | undefined,
  model: string | undefined,
  isConfigured: (provider: string) => boolean
): StageRoutingProblem | undefined {
  if (!provider || !isConfigured(provider)) return 'unconfigured';
  if (providerNeedsModel(provider) && !model) return 'no-model';
  return undefined;
}

/**
 * The problem with the ACTIVE provider, if any — the one every un-overridden
 * stage inherits. Reported ONCE, at card level, because the fix is one action
 * ("configure the provider"), not one per stage.
 */
export function resolveActiveProblem(input: {
  active: { provider?: string; model?: string };
  isConfigured: (provider: string) => boolean;
}): StageRoutingProblem | undefined {
  return routingProblem(input.active.provider, input.active.model, input.isConfigured);
}

/** One row per overridable stage, in pipeline order. */
export function resolveStageRouting(input: StageRoutingInput): StageRouting[] {
  const { overrides, active, isConfigured, stages = OVERRIDABLE_PIPELINE_STAGES } = input;

  return stages.map((stage) => {
    const override = overrides[stage];
    const provider = override?.provider ?? active.provider;
    const model = override?.model ?? active.model;

    return {
      stage,
      override,
      provider,
      model,
      // Only an override can make THIS ROW wrong; the active provider's own
      // problem belongs to `resolveActiveProblem`.
      problem: override
        ? routingProblem(override.provider, override.model, isConfigured)
        : undefined,
    };
  });
}
