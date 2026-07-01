import { Activity, Bookmark, Eye, FileText, type LucideIcon, Mail, Send } from 'lucide-react';

export interface Interaction {
  jobId: string;
  interactionType: string;
  timestamp: number;
  title: string;
  company: string;
  url: string;
  source: string;
  location: string;
}

/**
 * The three Documents lenses. Résumés + Cover Letters are lenses over the same
 * `AiGenerationRecord` set — each record bundles a résumé and an optional cover
 * letter — while Activity is the job-interaction log.
 */
export type DocTab = 'resumes' | 'coverLetters' | 'activity';

export const DOC_TABS: { id: DocTab; labelKey: string; icon: LucideIcon; color: string }[] = [
  { id: 'resumes', labelKey: 'resumes.tabs.resumes', icon: FileText, color: 'text-brand-soft' },
  { id: 'coverLetters', labelKey: 'resumes.tabs.coverLetters', icon: Mail, color: 'text-blue-300' },
  { id: 'activity', labelKey: 'resumes.tabs.activity', icon: Activity, color: 'text-emerald-300' },
];

/**
 * Per-interaction-type badge config — lets each InteractionRow describe its own
 * type in the mixed Activity feed (rather than inheriting one active-tab style).
 */
export interface InteractionTypeConfig {
  labelKey: string;
  icon: LucideIcon;
  color: string;
  ringColor: string;
}

export const INTERACTION_TYPES: Record<string, InteractionTypeConfig> = {
  applied: {
    labelKey: 'resumes.activity.applied',
    icon: Send,
    color: 'text-purple-300',
    ringColor: 'border-purple-400/30 bg-purple-400/10',
  },
  viewed: {
    labelKey: 'resumes.activity.viewed',
    icon: Eye,
    color: 'text-blue-300',
    ringColor: 'border-blue-400/30 bg-blue-400/10',
  },
  bookmarked: {
    labelKey: 'resumes.activity.bookmarked',
    icon: Bookmark,
    color: 'text-amber-300',
    ringColor: 'border-amber-400/30 bg-amber-400/10',
  },
};
