import {
  Briefcase,
  Contact,
  Cpu,
  FileText,
  Gauge,
  Languages,
  Lock,
  Shield,
  Terminal,
} from 'lucide-react';

export type SectionId =
  | 'general'
  | 'contact'
  | 'ai'
  | 'job'
  | 'resume'
  | 'accounts'
  | 'privacy'
  | 'performance'
  | 'developer';

export interface NavItem {
  id: SectionId;
  label: string;
  icon: React.ElementType;
  description: string;
}

export interface NavGroup {
  label: string;
  items: NavItem[];
}

export const NAV_GROUPS: NavGroup[] = [
  {
    label: 'settings.groups.preferences',
    items: [
      {
        id: 'general',
        label: 'settings.sections.general.label',
        icon: Languages,
        description: 'settings.sections.general.description',
      },
      {
        id: 'contact',
        label: 'settings.sections.contact.label',
        icon: Contact,
        description: 'settings.sections.contact.description',
      },
      {
        id: 'ai',
        label: 'settings.sections.ai.label',
        icon: Cpu,
        description: 'settings.sections.ai.description',
      },
      {
        id: 'job',
        label: 'settings.sections.jobs.label',
        icon: Briefcase,
        description: 'settings.sections.jobs.description',
      },
      {
        id: 'resume',
        label: 'settings.sections.resume.label',
        icon: FileText,
        description: 'settings.sections.resume.description',
      },
    ],
  },
  {
    label: 'settings.groups.system',
    items: [
      {
        id: 'accounts',
        label: 'settings.sections.accounts.label',
        icon: Lock,
        description: 'settings.sections.accounts.description',
      },
      {
        id: 'privacy',
        label: 'settings.sections.privacy.label',
        icon: Shield,
        description: 'settings.sections.privacy.description',
      },
      {
        id: 'performance',
        label: 'settings.sections.performance.label',
        icon: Gauge,
        description: 'settings.sections.performance.description',
      },
      {
        id: 'developer',
        label: 'settings.sections.developer.label',
        icon: Terminal,
        description: 'settings.sections.developer.description',
      },
    ],
  },
];
