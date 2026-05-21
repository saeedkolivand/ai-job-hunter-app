import {
  Briefcase,
  ChevronRight,
  Cpu,
  FileText,
  Gauge,
  Languages,
  Lock,
  Shield,
  User,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { Button, GlassCard, IconBadge, Input, SectionLabel } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { AccountsSettingsTab } from '@/features/settings/components/AccountsSettingsTab';
import { AISettingsTab } from '@/features/settings/components/AISettingsTab';
import { JobLocationPreferences } from '@/features/settings/components/JobLocationPreferences';
import { LanguageSelector } from '@/features/settings/components/LanguageSelector';
import { OutputTonePreferences } from '@/features/settings/components/OutputTonePreferences';
import { PerformancePreferences } from '@/features/settings/components/PerformancePreferences';
import { PrivacySettingsTab } from '@/features/settings/components/PrivacySettingsTab';
import { RemotePreferences } from '@/features/settings/components/RemotePreferences';
import { ResumePreferences } from '@/features/settings/components/ResumePreferences';
import { SalaryPreferences } from '@/features/settings/components/SalaryPreferences';
import { TechStackPreferences } from '@/features/settings/components/TechStackPreferences';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { transition, variants } from '@/lib/motion';
import {
  useOnboardingCompleted,
  usePreferencesStore,
  useUserName,
} from '@/store/preferences-store';

export const Route = createFileRoute('/settings')({ component: SettingsPage });

type SectionId = 'general' | 'ai' | 'job' | 'resume' | 'accounts' | 'privacy' | 'performance';

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
  const [activeSection, setActiveSection] = useState<SectionId>('general');
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
      ],
    },
  ];

  const allItems = navGroups.flatMap((g) => g.items);
  const current = allItems.find((i) => i.id === activeSection) as NavItem;

  return (
    <PageTransition className="flex h-full overflow-hidden">
      {/* ── Sidebar nav ─────────────────────────────────────────────── */}
      <aside className="flex w-56 shrink-0 flex-col gap-6 overflow-y-auto border-r border-white/[0.05] px-3 py-8">
        {navGroups.map((group) => (
          <div key={group.label}>
            <div className="mb-1.5 px-3 text-[10px] font-semibold uppercase tracking-widest text-foreground/30">
              {group.label}
            </div>
            <div className="flex flex-col gap-0.5">
              {group.items.map(({ id, label, icon: Icon }) => {
                const active = activeSection === id;
                return (
                  <Button
                    key={id}
                    variant="ghost"
                    onClick={() => setActiveSection(id)}
                    className={cn(
                      'group flex items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-all duration-150 justify-start',
                      active
                        ? 'bg-white/[0.07] text-foreground'
                        : 'text-foreground/50 hover:bg-white/[0.04] hover:text-foreground/80'
                    )}
                  >
                    <Icon
                      size={15}
                      className={cn(
                        'shrink-0 transition-colors',
                        active
                          ? 'text-brand-soft'
                          : 'text-foreground/35 group-hover:text-foreground/60'
                      )}
                    />
                    <span className="font-medium">{label}</span>
                    {active && <ChevronRight size={12} className="ml-auto text-foreground/30" />}
                  </Button>
                );
              })}
            </div>
          </div>
        ))}
      </aside>

      {/* ── Content ─────────────────────────────────────────────────── */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {/* Section header */}
        <div className="shrink-0 border-b border-white/[0.05] px-8 py-6">
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
            </motion.div>
          </AnimatePresence>
        </div>
      </div>
    </PageTransition>
  );
}

function GeneralSection({
  localName,
  setLocalName,
  setUserName,
  userName,
}: {
  localName: string;
  setLocalName: (v: string) => void;
  setUserName: (v: string) => void;
  userName: string | undefined;
}) {
  const { t } = useTranslation();
  const onboardingCompleted = useOnboardingCompleted();
  const replayWizard = () =>
    usePreferencesStore.setState((s) => ({ ...s, onboardingCompleted: false }));

  return (
    <>
      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={User} size="sm" />
          <SectionLabel>{t('settings.profile.title')}</SectionLabel>
        </div>
        <div className="space-y-3">
          <label className="block">
            <div className="mb-1.5 text-[11px] font-medium text-foreground/50">
              {t('settings.profile.displayName')}
            </div>
            <div className="flex gap-2">
              <Input
                value={localName}
                onChange={(e) => setLocalName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && localName !== (userName || '')) setUserName(localName);
                }}
                placeholder={t('settings.profile.placeholder')}
                className="flex-1"
              />
              <Button
                variant="glass"
                size="sm"
                onClick={() => setUserName(localName)}
                disabled={localName === (userName || '')}
              >
                {t('settings.profile.save')}
              </Button>
            </div>
          </label>
        </div>
      </GlassCard>

      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={Languages} size="sm" />
          <SectionLabel>{t('settings.language.title')}</SectionLabel>
        </div>
        <LanguageSelector />
      </GlassCard>

      <GlassCard>
        <div className="mb-4 flex items-center gap-2">
          <IconBadge icon={Wand2} size="sm" />
          <SectionLabel>{t('settings.onboarding.title')}</SectionLabel>
        </div>
        <div className="flex items-center justify-between">
          <p className="text-xs text-foreground/45">{t('settings.onboarding.description')}</p>
          <Button
            variant="glass"
            size="sm"
            onClick={replayWizard}
            disabled={!onboardingCompleted}
            className="ml-4 shrink-0"
          >
            {t('settings.onboarding.replay')}
          </Button>
        </div>
      </GlassCard>
    </>
  );
}
