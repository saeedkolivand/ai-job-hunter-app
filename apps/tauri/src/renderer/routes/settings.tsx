import {
  Briefcase,
  CheckCircle2,
  ChevronRight,
  Cpu,
  Download,
  FileText,
  Gauge,
  Languages,
  Loader2,
  Lock,
  Shield,
  Sparkles,
  Terminal,
  User,
  Wand2,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';

import { Button, GlassCard, IconBadge, Input, RefreshButton, SectionLabel } from '@ajh/ui';

import { PageTransition } from '@/components/layout/PageTransition';
import { AccountsSettingsTab } from '@/features/settings/components/AccountsSettingsTab';
import { AISettingsTab } from '@/features/settings/components/AISettingsTab';
import { DeveloperPreferences } from '@/features/settings/components/DeveloperPreferences';
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
import { useAppVersion } from '@/services';
import { useUpdater } from '@/services/use-updater';
import {
  useOnboardingCompleted,
  usePreferencesStore,
  useUserName,
} from '@/store/preferences-store';

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

const NO_RELEASE_PATTERNS = [
  'valid release json',
  'release json',
  'failed to fetch',
  'network',
  '404',
];

function isNoReleaseError(msg: string) {
  const lower = msg.toLowerCase();
  return NO_RELEASE_PATTERNS.some((p) => lower.includes(p));
}

function UpdateSection() {
  const { t } = useTranslation();
  const { data: versionRaw = '' } = useAppVersion();
  const version = versionRaw
    ? String(versionRaw).startsWith('v')
      ? String(versionRaw)
      : `v${versionRaw}`
    : '';
  const { status, check, download, install } = useUpdater();

  const noRelease = status.state === 'error' && isNoReleaseError(status.message);

  return (
    <GlassCard>
      <div className="mb-4 flex items-center gap-2">
        <IconBadge icon={Sparkles} size="sm" />
        <SectionLabel>{t('settings.update.title')}</SectionLabel>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs text-foreground/50">{t('settings.update.currentVersion')}</p>
          <p className="mt-0.5 font-mono text-sm text-foreground/80">{version || '—'}</p>
        </div>

        {status.state === 'idle' || status.state === 'not-available' || status.state === 'error' ? (
          <RefreshButton onRefresh={check} variant="glass" size={12} className="shrink-0 gap-2">
            {t('settings.update.checkNow')}
          </RefreshButton>
        ) : status.state === 'checking' ? (
          <div className="flex items-center gap-2 text-xs text-foreground/40">
            <Loader2 size={13} className="animate-spin" />
            {t('settings.update.checking')}
          </div>
        ) : status.state === 'available' ? (
          <Button
            variant="glass"
            size="sm"
            onClick={() => void download()}
            className="gap-2 ring-1 ring-brand/20"
          >
            <Download size={12} />
            {t('settings.update.download', { version: status.version })}
          </Button>
        ) : status.state === 'downloading' ? (
          <div className="flex items-center gap-2 text-xs text-foreground/40">
            <Loader2 size={13} className="animate-spin" />
            {status.percent}%
          </div>
        ) : status.state === 'downloaded' ? (
          <RefreshButton
            onRefresh={install}
            variant="glass"
            size={12}
            className="gap-2 ring-1 ring-brand/20"
          >
            {t('settings.update.install')}
          </RefreshButton>
        ) : null}
      </div>

      {/* Status messages */}
      {(status.state === 'not-available' || noRelease) && (
        <div className="mt-3 flex items-center gap-2 text-xs text-emerald-400/70">
          <CheckCircle2 size={12} />
          {t('settings.update.upToDate')}
        </div>
      )}
      {status.state === 'error' && !noRelease && (
        <div className="mt-3 text-xs text-red-400/70">
          {t('settings.update.error')}: {status.message}
        </div>
      )}

      {/* Changelog */}
      {(status.state === 'available' ||
        status.state === 'downloading' ||
        status.state === 'downloaded') &&
        'releaseNotes' in status &&
        status.releaseNotes && (
          <div className="mt-4 space-y-1.5">
            <div className="text-[10px] font-medium uppercase tracking-[0.16em] text-foreground/30">
              {t('settings.update.whatsNew')} {'version' in status ? status.version : ''}
            </div>
            <div className="max-h-36 overflow-y-auto rounded-lg border border-white/[0.05] bg-white/[0.02] p-3 text-xs text-foreground/50 leading-relaxed whitespace-pre-wrap">
              {status.releaseNotes}
            </div>
          </div>
        )}
    </GlassCard>
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

      <UpdateSection />
    </>
  );
}
