import { Bookmark, Eye, Send, Wand2 } from 'lucide-react';

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

export type Tab = 'applied' | 'viewed' | 'bookmarked' | 'generated';

export const TAB_CONFIG = [
  {
    id: 'applied' as Tab,
    labelKey: 'resumes.tabs.applied',
    icon: Send,
    color: 'text-purple-300',
    ringColor: 'border-purple-400/30 bg-purple-400/10',
  },
  {
    id: 'viewed' as Tab,
    labelKey: 'resumes.tabs.viewed',
    icon: Eye,
    color: 'text-blue-300',
    ringColor: 'border-blue-400/30 bg-blue-400/10',
  },
  {
    id: 'bookmarked' as Tab,
    labelKey: 'resumes.tabs.bookmarked',
    icon: Bookmark,
    color: 'text-amber-300',
    ringColor: 'border-amber-400/30 bg-amber-400/10',
  },
  {
    id: 'generated' as Tab,
    labelKey: 'resumes.tabs.generated',
    icon: Wand2,
    color: 'text-brand-soft',
    ringColor: 'border-brand/30 bg-brand/10',
  },
] as const;
