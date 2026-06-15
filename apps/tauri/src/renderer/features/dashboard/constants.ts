import { Briefcase, ClipboardList, FileText, Zap } from 'lucide-react';

export const QUICK_ACTIONS = [
  { icon: ClipboardList, labelKey: 'nav.applications', path: '/applications' },
  { icon: Zap, labelKey: 'nav.autopilot', path: '/autopilot' },
  { icon: Briefcase, labelKey: 'nav.jobs', path: '/jobs' },
  { icon: FileText, labelKey: 'nav.documents', path: '/documents' },
] as const;
