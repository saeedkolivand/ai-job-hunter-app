/**
 * Coarse match-strictness levels for the autopilot filter. The backend stores a
 * numeric `minMatchScore` (0–100); the wizard presents three discrete levels and
 * maps each to a canonical threshold. Selection is range-based so a legacy /
 * arbitrary stored score (e.g. an autopilot saved at 60 by the old slider) still
 * resolves to a level for display.
 */
export const MATCH_LEVELS = [
  { id: 'low', value: 30 },
  { id: 'medium', value: 50 },
  { id: 'high', value: 70 },
] as const;

export type MatchLevel = (typeof MATCH_LEVELS)[number]['id'];

/** Map a stored `minMatchScore` (0–100) to the level whose band contains it. */
export function scoreToLevel(score: number): MatchLevel {
  if (score < 40) return 'low';
  if (score < 65) return 'medium';
  return 'high';
}
