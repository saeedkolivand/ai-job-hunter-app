import type { PipelineRunEvent } from '@ajh/shared/ipc';

/**
 * Whether the run's `analyze_job` stage fell back to keyword-matched
 * requirements (both model attempts were below the quality floor, #1392).
 * Read from the persisted stage trail, not live events, so it survives
 * navigating away and back.
 */
export function usedKeywordFallback(events: readonly PipelineRunEvent[] | undefined): boolean {
  return (events ?? []).some(
    (e) =>
      e.stage === 'analyze_job' &&
      e.phase === 'finish' &&
      (e.artifact as { keywordFallback?: unknown } | null)?.keywordFallback === true
  );
}

/**
 * Whether the `draft` stage appended the source résumé's own Projects section
 * because the model dropped it (#1549). Same persisted-trail read as above.
 */
export function restoredSourceProjects(events: readonly PipelineRunEvent[] | undefined): boolean {
  return (events ?? []).some(
    (e) =>
      e.stage === 'draft' &&
      e.phase === 'finish' &&
      Array.isArray((e.artifact as { warnings?: unknown } | null)?.warnings) &&
      (e.artifact as { warnings: unknown[] }).warnings.includes('projects_restored_from_source')
  );
}
