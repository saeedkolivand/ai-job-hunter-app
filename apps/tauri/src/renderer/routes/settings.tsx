import {
  Briefcase,
  ChevronRight,
  Cpu,
  FileText,
  Gauge,
  Languages,
  Lock,
  Shield,
  Terminal,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { cn, IconBadge, transition, variants } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { AccountsSettingsTab } from '@/features/settings/components/AccountsSettingsTab';
import { AISettingsTab } from '@/features/settings/components/AISettingsTab';
import { DeveloperPreferences } from '@/features/settings/components/DeveloperPreferences';
import { GeneralSection } from '@/features/settings/components/GeneralSection';
import { JobLocationPreferences } from '@/features/settings/components/JobLocationPreferences';
import { OutputTonePreferences } from '@/features/settings/components/OutputTonePreferences';
import { PerformancePreferences } from '@/features/settings/components/PerformancePreferences';
import { PrivacySettingsTab } from '@/features/settings/components/PrivacySettingsTab';
import { RemotePreferences } from '@/features/settings/components/RemotePreferences';
import { ResumePreferences } from '@/features/settings/components/ResumePreferences';
import { SalaryPreferences } from '@/features/settings/components/SalaryPreferences';
import { TechStackPreferences } from '@/features/settings/components/TechStackPreferences';
import { useTranslation } from '@/lib/i18n';
import { usePreferencesStore, useUserName } from '@/store/preferences-store';
import { useSessionStore } from '@/store/session-store';

export const Route = createFileRoute('/settings')({ component: SettingsPage });

type SectionId =
  | 'general'
  | 'ai'
  | 'job'
  | 'resume'
  | 'accounts'
  | 'privacy'
  | 'performance'
  | 'developer';

interface NavItem {
  id: SectionId;
  label: string;
  icon: React.ElementType;
  description: string;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

function SettingsPage() {
  const { t } = useTranslation();
  const { settings, setSettings } = useSessionStore();
  const activeSection = settings.activeSection as SectionId;
  const setActiveSection = (v: SectionId) => setSettings({ activeSection: v });
  const userName = useUserName();
  const setUserName = usePreferencesStore((s) => s.setUserName);
  const [localName, setLocalName] = useState(userName || '');

  const navGroups: NavGroup[] = [
    {
      label: t('settings.groups.preferences'),
      items: [
        {
          id: 'general',
          label: t('settings.sections.general.label'),
          icon: Languages,
          description: t('settings.sections.general.description'),
        },
        {
          id: 'ai',
          label: t('settings.sections.ai.label'),
          icon: Cpu,
          description: t('settings.sections.ai.description'),
        },
        {
          id: 'job',
          label: t('settings.sections.jobs.label'),
          icon: Briefcase,
          description: t('settings.sections.jobs.description'),
        },
        {
          id: 'resume',
          label: t('settings.sections.resume.label'),
          icon: FileText,
          description: t('settings.sections.resume.description'),
        },
      ],
    },
    {
      label: t('settings.groups.system'),
      items: [
        {
          id: 'accounts',
          label: t('settings.sections.accounts.label'),
          icon: Lock,
          description: t('settings.sections.accounts.description'),
        },
        {
          id: 'privacy',
          label: t('settings.sections.privacy.label'),
          icon: Shield,
          description: t('settings.sections.privacy.description'),
        },
        {
          id: 'performance',
          label: t('settings.sections.performance.label'),
          icon: Gauge,
          description: t('settings.sections.performance.description'),
        },
        {
          id: 'developer',
          label: t('settings.sections.developer.label'),
          icon: Terminal,
          description: t('settings.sections.developer.description'),
        },
      ],
    },
  ];

  const allItems = navGroups.flatMap((g) => g.items);
  const current = allItems.find((i) => i.id === activeSection) as NavItem;

  return (
    <PageTransition className="flex h-full overflow-hidden">
      {/* ── Sidebar nav ─────────────────────────────────────────────── */}
      <aside className="flex w-56 shrink-0 flex-col gap-6 overflow-y-auto border-white/[0.05] px-3 py-8">
        {navGroups.map((group) => (
          <div key={group.label}>
            <div className="mb-1.5 px-3 text-[10px] font-semibold uppercase tracking-widest text-foreground/30">
              {group.label}
            </div>
            <nav className="flex flex-col gap-1">
              {group.items.map(({ id, label, icon: Icon }) => {
                const active = activeSection === id;
                return (
                  <div key={id} className="relative">
                    {active && (
                      <motion.div
                        layoutId="settings-pill"
                        className="absolute inset-0 rounded-xl bg-white/[0.07]"
                        transition={transition.spring}
                      />
                    )}
                    <div
                      role="button"
                      onClick={() => setActiveSection(id)}
                      className={cn(
                        'group relative flex cursor-pointer items-center gap-2.5 rounded-xl px-3 py-2 text-sm transition-colors duration-150',
                        active
                          ? 'text-foreground'
                          : 'text-foreground/45 hover:bg-white/[0.04] hover:text-foreground/75'
                      )}
                    >
                      <Icon
                        size={15}
                        className={cn(
                          'shrink-0 transition-colors duration-150',
                          active
                            ? 'text-foreground/70'
                            : 'text-foreground/35 group-hover:text-foreground/55'
                        )}
                      />
                      <span className="flex-1 font-medium">{label}</span>
                      {active && <ChevronRight size={12} className="text-foreground/30" />}
                    </div>
                  </div>
                );
              })}
            </nav>
          </div>
        ))}
      </aside>

      {/* ── Content ─────────────────────────────────────────────────── */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {/* Section header */}
        <div className="shrink-0 border-white/[0.05] px-8 py-6">
          <div className="flex items-center gap-3">
            <IconBadge icon={current.icon} size="md" />
            <div>
              <div className="text-base font-semibold text-foreground/90">{current.label}</div>
              <div className="text-[11px] text-foreground/40">{current.description}</div>
            </div>
          </div>
        </div>

        {/* Scrollable content */}
        <div className="flex-1 overflow-y-auto px-8 py-6">
          <AnimatePresence mode="wait">
            <motion.div
              key={activeSection}
              {...variants.fadeSlideUp}
              transition={transition.normal}
              className="max-w-2xl space-y-4"
            >
              {activeSection === 'general' && (
                <GeneralSection
                  localName={localName}
                  setLocalName={setLocalName}
                  setUserName={setUserName}
                  userName={userName}
                />
              )}
              {activeSection === 'ai' && (
                <>
                  <AISettingsTab />
                  <OutputTonePreferences />
                </>
              )}
              {activeSection === 'job' && (
                <>
                  <JobLocationPreferences />
                  <RemotePreferences />
                  <TechStackPreferences />
                  <SalaryPreferences />
                </>
              )}
              {activeSection === 'resume' && <ResumePreferences />}
              {activeSection === 'accounts' && <AccountsSettingsTab />}
              {activeSection === 'privacy' && <PrivacySettingsTab />}
              {activeSection === 'performance' && <PerformancePreferences />}
              {activeSection === 'developer' && <DeveloperPreferences />}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </PageTransition>
  );
}
