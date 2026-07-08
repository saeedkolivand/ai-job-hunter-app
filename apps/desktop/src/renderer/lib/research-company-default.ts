/**
 * The core decision for the capability-driven "search company" toggle default,
 * shared by every consumer so the little state machine can't drift between them.
 * Pure (no React) → unit-testable in isolation. Each consumer still owns its own
 * state plumbing (`useState` vs react-hook-form); only the DECISION lives here.
 */
export interface SeedResearchDefaultInput {
  /** The active-model capability query has resolved (success). */
  capabilityResolved: boolean;
  /** The active model can web-search — the value the toggle should default to. */
  supportsWebSearch: boolean;
  /** The user has manually toggled the control; their explicit choice is sticky. */
  userTouched: boolean;
  /**
   * The capability value written by the last seed, or `null` if never seeded. A
   * tracked value (not a one-shot flag) so a mid-session model switch that FLIPS
   * the capability re-seeds, while a resolve that doesn't change it is a no-op.
   */
  lastSeededValue: boolean | null;
}

export interface SeedResearchDefaultDecision {
  /** Apply `value` to the toggle. */
  seed: boolean;
  /** The value to write when `seed` is true. */
  value: boolean;
}

/**
 * Decide whether to (re)seed the "search company" toggle from the active model's
 * web-search capability:
 * - an explicit user choice always wins (never clobbered),
 * - nothing seeds until the capability resolves,
 * - it seeds on the first resolve AND re-seeds whenever a mid-session model
 *   switch flips the capability — but never re-writes a value already applied.
 */
export function shouldSeedResearchDefault({
  capabilityResolved,
  supportsWebSearch,
  userTouched,
  lastSeededValue,
}: SeedResearchDefaultInput): SeedResearchDefaultDecision {
  if (userTouched || !capabilityResolved) return { seed: false, value: supportsWebSearch };
  return { seed: lastSeededValue !== supportsWebSearch, value: supportsWebSearch };
}
