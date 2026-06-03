import {
  Activity,
  Briefcase,
  Cpu,
  FileText,
  Gauge,
  HelpCircle,
  LayoutDashboard,
  type LucideIcon,
  Settings,
  Sparkles,
  User,
  Wand2,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';
import { Link, useRouterState } from '@tanstack/react-router';

import { Button, cn, NavPill, transition, variants } from '@ajh/ui';

import { ROUTES } from '@/constants/routes';
import { getTimeGreeting } from '@/lib/greeting';
import { useTranslation } from '@/lib/i18n';
import { useAICapability } from '@/providers/CapabilityProvider';
import { useAppVersion } from '@/services/use-system';
import { useAIModel, useAiProviderConfig, useUserName } from '@/store/preferences-store';

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  tourId: string;
}

// Scrollable nav, grouped by intent. Search is intentionally absent — it lives
// on ⌘/Ctrl+K (and the dashboard quick-links); its route is unchanged.
const NAV_SECTIONS: { labelKey: string; items: readonly NavItem[] }[] = [
  {
    labelKey: 'nav.sections.workspace',
    items: [
      { to: ROUTES.DASHBOARD, label: 'nav.dashboard', icon: LayoutDashboard, tourId: 'dashboard' },
      { to: ROUTES.JOBS, label: 'nav.jobs', icon: Briefcase, tourId: 'jobs' },
      { to: ROUTES.ANALYZE, label: 'nav.analyze', icon: Gauge, tourId: 'analyze' },
      { to: ROUTES.GENERATE, label: 'nav.generate', icon: Wand2, tourId: 'generate' },
      // tourId 'documents' matches the onboarding tour (the visible label is
      // relabelled "Documents" in the IA pass; the route stays /resumes).
      { to: ROUTES.RESUMES, label: 'nav.resumes', icon: FileText, tourId: 'documents' },
    ],
  },
  {
    labelKey: 'nav.sections.automation',
    items: [
      { to: ROUTES.AUTOPILOT, label: 'nav.autopilot', icon: Zap, tourId: 'autopilot' },
      { to: ROUTES.MONITORING, label: 'nav.monitoring', icon: Activity, tourId: 'monitoring' },
    ],
  },
];

// Pinned to the bottom of the nav, above the user/status footer.
const PINNED_ITEMS: readonly NavItem[] = [
  { to: ROUTES.AI, label: 'nav.ai', icon: Sparkles, tourId: 'ai' },
  { to: ROUTES.SUPPORT, label: 'nav.support', icon: HelpCircle, tourId: 'support' },
  { to: ROUTES.SETTINGS, label: 'nav.settings', icon: Settings, tourId: 'settings' },
];

export function Sidebar() {
  const { t } = useTranslation();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const userName = useUserName();
  const aiModel = useAIModel();
  const providerConfig = useAiProviderConfig();
  const ai = useAICapability(); // from CapabilityProvider — no duplicate polling
  const { data: version = 'v0.1.0' } = useAppVersion();
  const appVersion = version.startsWith('v') ? version : `v${version}`;

  const [versionTooltip, setVersionTooltip] = useState(false);
  const tooltipTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const aiStatus = !ai ? 'checking' : ai.ready ? 'ready' : 'offline';

  // Get current model name from active provider
  const activeProvider = providerConfig?.activeProvider ?? 'ollama';
  const currentModel =
    activeProvider === 'ollama'
      ? aiModel?.defaultModel
      : providerConfig?.providers?.[activeProvider]?.model;

  const showVersion = () => {
    setVersionTooltip(true);
    if (tooltipTimer.current) clearTimeout(tooltipTimer.current);
    tooltipTimer.current = setTimeout(() => setVersionTooltip(false), 2000);
  };

  const renderNavItem = ({ to, label, icon: Icon, tourId }: NavItem) => {
    const active = pathname === to || (to !== '/' && pathname.startsWith(to + '/'));
    return (
      <div key={to} className="relative" data-tour-id={tourId}>
        {active && <NavPill layoutId="sidebar-pill" />}
        <Link
          to={to}
          className={cn(
            'group relative flex items-center gap-2.5 rounded-xl px-3 py-2 text-sm transition-colors duration-150',
            active
              ? 'text-foreground'
              : 'text-foreground/45 hover:bg-foreground/[0.04] hover:text-foreground/75'
          )}
        >
          <Icon
            size={15}
            className={cn(
              'shrink-0 transition-colors duration-150',
              active ? 'text-brand-soft' : 'text-foreground/35 group-hover:text-foreground/55'
            )}
          />
          <span className="flex-1 font-medium">{t(label)}</span>
        </Link>
      </div>
    );
  };

  return (
    <aside className="app-sidebar glass-surface m-3 mr-0 flex w-60 flex-col rounded-2xl p-3">
      <nav className="flex flex-col gap-4">
        {NAV_SECTIONS.map((section) => (
          <div key={section.labelKey}>
            <div className="mb-1 px-3 text-[10px] font-semibold uppercase tracking-widest text-foreground/40">
              {t(section.labelKey)}
            </div>
            <div className="flex flex-col gap-1">{section.items.map(renderNavItem)}</div>
          </div>
        ))}
      </nav>

      {/* Pinned: AI config + support + settings, anchored to the bottom. */}
      <nav className="mt-auto flex flex-col gap-1 border-t border-white/[0.06] pb-3 pt-3">
        {PINNED_ITEMS.map(renderNavItem)}
      </nav>

      <div className="space-y-2 px-3 pb-3">
        {userName && (
          <div className="flex items-center gap-2.5 px-1">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-brand-soft/25 to-brand/10 ring-1 ring-brand/20">
              <User size={15} className="text-brand-soft" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium leading-tight text-foreground/85">
                {userName}
              </div>
              <div className="text-xs leading-tight text-foreground/40">{getTimeGreeting()}</div>
            </div>
            <Link
              to={ROUTES.SETTINGS}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-foreground/30 transition-colors hover:bg-foreground/[0.06] hover:text-foreground/60"
            >
              <Settings size={12} />
            </Link>
          </div>
        )}

        <div className="flex items-center gap-2 rounded-lg border border-foreground/[0.06] bg-foreground/[0.03] px-2 py-1.5">
          <Cpu
            size={11}
            className={cn(
              'shrink-0',
              aiStatus === 'ready'
                ? 'text-emerald-400/70'
                : aiStatus === 'offline'
                  ? 'text-red-400/60'
                  : 'text-foreground/25'
            )}
          />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span
                className={cn(
                  'h-1.5 w-1.5 shrink-0 rounded-full',
                  aiStatus === 'ready'
                    ? 'bg-emerald-400 shadow-[0_0_4px_rgba(52,211,153,0.6)]'
                    : aiStatus === 'offline'
                      ? 'bg-red-400/70'
                      : 'animate-pulse bg-foreground/20'
                )}
              />
              <span className="truncate text-xs text-foreground/55">
                {aiStatus === 'ready'
                  ? currentModel
                    ? currentModel.length > 30
                      ? `${currentModel.slice(0, 28)}…`
                      : currentModel
                    : 'AI ready'
                  : aiStatus === 'offline'
                    ? 'AI offline'
                    : 'Checking…'}
              </span>
            </div>
          </div>
        </div>

        <div className="relative flex justify-center">
          <Button
            variant="unstyled"
            onClick={showVersion}
            className="font-mono text-[11px] tabular-nums text-foreground/25 transition-colors hover:text-foreground/45"
          >
            {appVersion}
          </Button>
          <AnimatePresence>
            {versionTooltip && (
              <motion.div
                {...variants.fadeSlideDown}
                transition={transition.fast}
                className="absolute bottom-full mb-1.5 whitespace-nowrap rounded-lg border border-border bg-secondary px-2.5 py-1.5 text-xs text-foreground/60 shadow-xl"
              >
                {appVersion} · local build
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </aside>
  );
}
