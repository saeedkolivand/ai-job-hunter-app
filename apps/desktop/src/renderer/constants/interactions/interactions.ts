/**
 * Interaction types that count as "tracked".
 *
 * An explicit allowlist — not "every type except `dismissed`" — so a future
 * SIXTH interaction type must be deliberately added here before it can
 * silently inflate a headline number. `dismissed` is excluded on purpose:
 * a job the user explicitly rejected was never "tracked".
 *
 * Read by `JobPipelineOverview` (the "total tracked" stat), `NextStepTile`
 * (whether the job step is met) and the help chat's data glance
 * (`use-help-chat`), so none of them can disagree about what tracking means —
 * the tile counted dismissals as progress while the card next to it said zero.
 *
 * Lives in `constants/` rather than `features/dashboard/` because it is now
 * read from two different features and rule 9 forbids importing across feature
 * directories.
 */
export const TRACKED_INTERACTION_TYPES = new Set(['viewed', 'opened', 'applied', 'bookmarked']);
