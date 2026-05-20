import {
  Activity,
  AlertTriangle,
  Cpu,
  FileText,
  HelpCircle,
  LogOut,
  RefreshCw,
  Search,
  Settings,
} from 'lucide-react';

export type TabId =
  | 'health'
  | 'ai'
  | 'documents'
  | 'scraping'
  | 'performance'
  | 'logs'
  | 'recovery'
  | 'knowledge'
  | 'contact';

export const SUPPORT_TABS = [
  { id: 'health' as TabId, icon: Activity, labelKey: 'support.tabs.health' },
  { id: 'ai' as TabId, icon: Cpu, labelKey: 'support.tabs.ai' },
  { id: 'documents' as TabId, icon: FileText, labelKey: 'support.tabs.documents' },
  { id: 'scraping' as TabId, icon: Search, labelKey: 'support.tabs.scraping' },
  { id: 'performance' as TabId, icon: Settings, labelKey: 'support.tabs.performance' },
  { id: 'logs' as TabId, icon: LogOut, labelKey: 'support.tabs.logs' },
  { id: 'recovery' as TabId, icon: RefreshCw, labelKey: 'support.tabs.recovery' },
  { id: 'knowledge' as TabId, icon: HelpCircle, labelKey: 'support.tabs.knowledge' },
  { id: 'contact' as TabId, icon: AlertTriangle, labelKey: 'support.tabs.contact' },
];
