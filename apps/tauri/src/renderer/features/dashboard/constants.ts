import { Briefcase, FileText, Search } from 'lucide-react';

export const QUICK_ACTIONS = [
  { icon: Briefcase, labelKey: 'nav.jobs', path: '/jobs' },
  { icon: FileText, labelKey: 'nav.resumes', path: '/documents' },
  { icon: Search, labelKey: 'nav.search', path: '/search' },
] as const;
