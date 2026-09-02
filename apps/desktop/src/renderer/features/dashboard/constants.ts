import { Briefcase, ClipboardList, FileText, Zap } from 'lucide-react';

export const QUICK_ACTIONS = [
  { icon: ClipboardList, labelKey: 'nav.applications', path: '/applications' },
  { icon: Zap, labelKey: 'nav.autopilot', path: '/autopilot' },
  { icon: Briefcase, labelKey: 'nav.jobs', path: '/jobs' },
  { icon: FileText, labelKey: 'nav.documents', path: '/documents' },
] as const;

/**
 * Interaction types that count toward "tracked" on the Dashboard.
 * An explicit allowlist — not "every type except `dismissed`" — so a future
 * SIXTH interaction type must be deliberately added here before it can
 * silently inflate a headline number. `dismissed` is excluded on purpose:
 * a job the user explicitly rejected was never "tracked".
 *
 * Shared by `JobPipelineOverview` (the "total tracked" stat) and `NextStepTile`
 * (whether the job step is met) so the two cannot disagree about what tracking
 * means — the tile counted dismissals as progress while the card next to it
 * said zero.
 */
export const TRACKED_INTERACTION_TYPES = new Set(['viewed', 'opened', 'applied', 'bookmarked']);
