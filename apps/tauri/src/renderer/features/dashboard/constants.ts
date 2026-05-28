import { Briefcase, FileText, Search, Sparkles } from 'lucide-react';

export const QUICK_ACTIONS = [
  { icon: Sparkles, labelKey: 'nav.ai', path: '/ai' },
  { icon: Briefcase, labelKey: 'nav.jobs', path: '/jobs' },
  { icon: FileText, labelKey: 'nav.resumes', path: '/resumes' },
  { icon: Search, labelKey: 'nav.search', path: '/search' },
] as const;
