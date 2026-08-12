import type { AiSpendModelThinking, AiStageOverride } from '@ajh/shared';
import type { ModelInspectResult } from '@ajh/shared/schemas';

/**
 * Everything the advisor's steps read, resolved ONCE by the wizard shell.
 *
 * Passed down rather than re-queried per step so all four steps describe the
 * same machine: a step that refetched could contradict the step before it.
 */
export interface AdvisorContext {
  activeProvider?: string;
  activeModel?: string;
  /** Installed local models (`/api/tags`) — names only; the window comes from
   *  `inspections`, because `/api/tags` does not carry one. */
  installedModels: string[];
  /** `/api/show` per model. `null` = Ollama could not describe it, which is
   *  NOT MEASURED rather than "no window". */
  inspections: Record<string, ModelInspectResult | null>;
  inspectionsPending: boolean;
  /** Current per-stage overrides — the advisor recommends around what is
   *  already pinned and never re-suggests a pinned stage. */
  overrides: Record<string, AiStageOverride>;
  /** The active provider's configured reasoning effort, when it has one. */
  effort?: string;
  /** `spendSummary().thinkingByModel` — EMPTY for a local-only user. */
  thinkingByModel: AiSpendModelThinking[];
}

export interface AdvisorStepProps {
  ctx: AdvisorContext;
}
