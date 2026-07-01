import { Briefcase, Cpu, FileText, Languages, Lock, Shield, Zap } from 'lucide-react';

export type TabId = 'general' | 'ai' | 'accounts' | 'job' | 'resume' | 'performance' | 'privacy';

export const TAB_IDS = {
  GENERAL: 'general' as TabId,
  AI: 'ai' as TabId,
  ACCOUNTS: 'accounts' as TabId,
  JOB: 'job' as TabId,
  RESUME: 'resume' as TabId,
  PERFORMANCE: 'performance' as TabId,
  PRIVACY: 'privacy' as TabId,
};

export const TABS = [
  { id: TAB_IDS.GENERAL, label: 'General', icon: Languages },
  { id: TAB_IDS.AI, label: 'AI', icon: Cpu },
  { id: TAB_IDS.JOB, label: 'Jobs', icon: Briefcase },
  { id: TAB_IDS.RESUME, label: 'Resume', icon: FileText },
  { id: TAB_IDS.PERFORMANCE, label: 'Performance', icon: Zap },
  { id: TAB_IDS.ACCOUNTS, label: 'Accounts', icon: Lock },
  { id: TAB_IDS.PRIVACY, label: 'Privacy', icon: Shield },
] as const;

export const TAB_GROUPS = [
  {
    label: 'AI Preferences',
    tabs: [TAB_IDS.AI],
  },
  {
    label: 'Job Preferences',
    tabs: [TAB_IDS.JOB],
  },
  {
    label: 'Resume Preferences',
    tabs: [TAB_IDS.RESUME],
  },
  {
    label: 'System Preferences',
    tabs: [TAB_IDS.GENERAL, TAB_IDS.PERFORMANCE, TAB_IDS.ACCOUNTS, TAB_IDS.PRIVACY],
  },
] as const;
