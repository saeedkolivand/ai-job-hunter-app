import { useState, useRef } from 'react';
import { Link, useRouterState } from '@tanstack/react-router';
import { useTranslation } from '@/lib/i18n';
import { motion, AnimatePresence } from 'motion/react';
import {
  LayoutDashboard,
  Briefcase,
  FileText,
  Search,
  Sparkles,
  Settings,
  Gauge,
  Activity,
  Wand2,
  User,
  HelpCircle,
  Cpu,
  Zap,
} from 'lucide-react';
import { cn } from '@/lib/cn';
import { ROUTES } from '@/constants/routes';
import { useUserName } from '@/store/preferences-store';
import { getTimeGreeting } from '@/lib/greeting';
import { transition, variants } from '@/lib/motion';
import { useAICapability } from '@/providers/CapabilityProvider';
import { useAppVersion } from '@/services/use-system';

const NAV_ITEMS = [
  { to: ROUTES.DASHBOARD, label: 'nav.dashboard', icon: LayoutDashboard, tourId: 'dashboard' },
  { to: ROUTES.ANALYZE, label: 'nav.analyze', icon: Gauge, tourId: 'analyze' },
  { to: ROUTES.GENERATE, label: 'nav.generate', icon: Wand2, tourId: 'generate' },
  { to: ROUTES.JOBS, label: 'nav.jobs', icon: Briefcase, tourId: 'jobs' },
  { to: ROUTES.AUTOPILOT, label: 'nav.autopilot', icon: Zap, tourId: 'autopilot' },
  { to: ROUTES.RESUMES, label: 'nav.resumes', icon: FileText, tourId: 'resumes' },
  { to: ROUTES.SEARCH, label: 'nav.search', icon: Search, tourId: 'search' },
  { to: ROUTES.AI, label: 'nav.ai', icon: Sparkles, tourId: 'ai' },
  { to: ROUTES.MONITORING, label: 'nav.monitoring', icon: Activity, tourId: 'monitoring' },
  { to: ROUTES.SUPPORT, label: 'nav.support', icon: HelpCircle, tourId: 'support' },
  { to: ROUTES.SETTINGS, label: 'nav.settings', icon: Settings, tourId: 'settings' },
] as const;

export function Sidebar() {
  const { t } = useTranslation();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const userName = useUserName();
  const ai = useAICapability(); // from CapabilityProvider — no duplicate polling
  const { data: version = 'v0.1.0' } = useAppVersion();
  const appVersion = version.startsWith('v') ? version : `v${version}`;

  const [versionTooltip, setVersionTooltip] = useState(false);
  const tooltipTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const aiStatus = !ai ? 'checking' : ai.ready ? 'ready' : 'offline';

  const showVersion = () => {
    setVersionTooltip(true);
    if (tooltipTimer.current) clearTimeout(tooltipTimer.current);
    tooltipTimer.current = setTimeout(() => setVersionTooltip(false), 2000);
  };

  return (
    <aside className="glass-surface m-3 mr-0 flex w-60 flex-col rounded-2xl p-3">
      <nav className="flex flex-col gap-1">
        {NAV_ITEMS.map(({ to, label, icon: Icon, tourId }) => {
          const active = pathname === to || (to !== '/' && pathname.startsWith(to + '/'));
          return (
            <div key={to} className="relative" data-tour-id={tourId}>
              {active && (
                <motion.div
                  layoutId="sidebar-pill"
                  className="absolute inset-0 rounded-xl"
                  style={{
                    background:
                      'linear-gradient(135deg, rgba(168,85,247,0.18) 0%, rgba(99,102,241,0.10) 100%)',
                    border: '1px solid rgba(168,85,247,0.25)',
                    boxShadow: '0 0 16px rgba(168,85,247,0.12)',
                  }}
                  transition={transition.spring}
                />
              )}
              <Link
                to={to}
                className={cn(
                  'group relative flex items-center gap-2.5 rounded-xl px-3 py-2 text-sm transition-colors duration-150',
                  active
                    ? 'text-foreground'
                    : 'text-foreground/45 hover:bg-white/[0.04] hover:text-foreground/75'
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
        })}
      </nav>

      <div className="mt-auto space-y-2 border-t border-white/[0.06] px-3 pb-3 pt-3">
        {userName && (
          <div className="flex items-center gap-2.5 px-1">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-brand-soft/25 to-brand/10 ring-1 ring-brand/20">
              <User size={15} className="text-brand-soft" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-medium leading-tight text-foreground/85">
                {userName}
              </div>
              <div className="text-[10px] leading-tight text-foreground/35">
                {getTimeGreeting()}
              </div>
            </div>
            <Link
              to={ROUTES.SETTINGS}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-foreground/30 transition-colors hover:bg-white/[0.06] hover:text-foreground/60"
            >
              <Settings size={12} />
            </Link>
          </div>
        )}

        <div className="flex items-center gap-2 rounded-lg border border-white/[0.05] bg-white/[0.03] px-2 py-1.5">
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
              <span className="truncate text-[10px] text-foreground/45">
                {aiStatus === 'ready'
                  ? ai.model
                    ? ai.model.split(':')[0]
                    : 'Ollama ready'
                  : aiStatus === 'offline'
                    ? 'Ollama offline'
                    : 'Checking…'}
              </span>
            </div>
          </div>

          <div className="relative">
            <button
              onClick={showVersion}
              className="font-mono text-[9px] tabular-nums text-foreground/20 transition-colors hover:text-foreground/40"
            >
              {appVersion}
            </button>
            <AnimatePresence>
              {versionTooltip && (
                <motion.div
                  {...variants.fadeSlideDown}
                  transition={transition.fast}
                  className="absolute bottom-full right-0 mb-1.5 whitespace-nowrap rounded-lg border border-white/10 bg-secondary px-2.5 py-1.5 text-[10px] text-foreground/60 shadow-xl"
                >
                  {appVersion} · local build
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </div>
    </aside>
  );
}
