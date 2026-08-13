import type {
  AiSpendModelThinking,
  AiStageOverride,
  ModelInspectResult,
  PipelineStage,
} from '@ajh/shared';

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
  overrides: Partial<Record<PipelineStage, AiStageOverride>>;
  /** The active provider's configured reasoning effort, when it has one. */
  effort?: string;
  /** `spendSummary().thinkingByModel` — EMPTY for a local-only user. */
  thinkingByModel: AiSpendModelThinking[];
  /**
   * Move focus to the wizard's primary footer control.
   *
   * A step that unmounts the control the user just pressed has to hand focus to
   * something that still exists AND is inside the focus trap — `<body>` is
   * neither, and the trap's keydown listener lives on the panel, so a
   * body-focused Tab walks straight out to the page behind the overlay.
   */
  focusPrimaryAction?: () => void;
}

export interface AdvisorStepProps {
  ctx: AdvisorContext;
}
