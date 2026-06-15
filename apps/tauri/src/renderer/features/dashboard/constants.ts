import { Briefcase, FileText } from 'lucide-react';

export const QUICK_ACTIONS = [
  { icon: Briefcase, labelKey: 'nav.jobs', path: '/jobs' },
  { icon: FileText, labelKey: 'nav.documents', path: '/documents' },
] as const;
