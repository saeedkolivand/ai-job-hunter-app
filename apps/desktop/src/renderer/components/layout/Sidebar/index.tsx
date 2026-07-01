import {
  Activity,
  Briefcase,
  ClipboardList,
  FilePlus2,
  FileText,
  Gauge,
  HelpCircle,
  LayoutDashboard,
  type LucideIcon,
  PanelLeftClose,
  Settings,
  User,
  Wand2,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useRef, useState } from 'react';
import { Link, useRouterState } from '@tanstack/react-router';

import { useTranslation } from '@ajh/translations';
import { Button, cn, Image, NavPill, transition, variants } from '@ajh/ui';

import { ROUTES } from '@/constants/routes';
import { getTimeGreeting } from '@/lib/greeting';
import { TOOLTIP_HIDE_MS } from '@/lib/timings';
import { useContactProfile } from '@/services';
import { useAppVersion } from '@/services/use-system';
import { useToggleSidebar, useUserName } from '@/store/preferences-store';

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  tourId: string;
}
const NAV_SECTIONS: { labelKey: string; items: readonly NavItem[] }[] = [
  {
    labelKey: 'nav.sections.workspace',
    items: [
      { to: ROUTES.DASHBOARD, label: 'nav.dashboard', icon: LayoutDashboard, tourId: 'dashboard' },
      {
        to: ROUTES.APPLICATIONS,
        label: 'nav.applications',
        icon: ClipboardList,
        tourId: 'applications',
      },
      { to: ROUTES.JOBS, label: 'nav.jobs', icon: Briefcase, tourId: 'jobs' },
      { to: ROUTES.ANALYZE, label: 'nav.analyze', icon: Gauge, tourId: 'analyze' },
      { to: ROUTES.GENERATE, label: 'nav.generate', icon: Wand2, tourId: 'generate' },
      { to: ROUTES.BUILD, label: 'nav.build', icon: FilePlus2, tourId: 'build' },
      { to: ROUTES.RESUMES, label: 'nav.documents', icon: FileText, tourId: 'documents' },
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

const PINNED_ITEMS: readonly NavItem[] = [
  { to: ROUTES.SUPPORT, label: 'nav.support', icon: HelpCircle, tourId: 'support' },
  { to: ROUTES.SETTINGS, label: 'nav.settings', icon: Settings, tourId: 'settings' },
];

// ponytail: query the page's scroll regions by their Tailwind class instead
// of wiring a ref registry through every route — one nav action doesn't need that.
function scrollPageToTop() {
  document
    .querySelector('main.app-main')
    ?.querySelectorAll('.overflow-y-auto')
    .forEach((el) => el.scrollTo({ top: 0, behavior: 'smooth' }));
}

export function Sidebar() {
  const { t } = useTranslation();
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const userName = useUserName();
  // Use the contact-profile photo (a local data: URL) as the footer avatar when set.
  const { data: contactProfile } = useContactProfile();
  const avatar = contactProfile?.photo;

  const { data: version = 'v0.1.0' } = useAppVersion();
  const appVersion = version.startsWith('v') ? version : `v${version}`;

  const toggleSidebar = useToggleSidebar();

  const [versionTooltip, setVersionTooltip] = useState(false);
  const tooltipTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showVersion = () => {
    setVersionTooltip(true);
    if (tooltipTimer.current) clearTimeout(tooltipTimer.current);
    tooltipTimer.current = setTimeout(() => setVersionTooltip(false), TOOLTIP_HIDE_MS);
  };

  const renderNavItem = ({ to, label, icon: Icon, tourId }: NavItem) => {
    const active = pathname === to || (to !== '/' && pathname.startsWith(to + '/'));
    const linkClassName = cn(
      'group relative flex items-center gap-2.5 rounded-xl px-3 py-2 text-sm transition-colors duration-150',
      active
        ? 'text-foreground'
        : 'text-foreground/45 hover:bg-foreground/[0.04] hover:text-foreground/75'
    );
    const linkContent = (
      <>
        <Icon
          size={15}
          className={cn(
            'shrink-0 transition-colors duration-150',
            active ? 'text-brand-soft' : 'text-foreground/35 group-hover:text-foreground/55'
          )}
        />
        <span className="flex-1 font-medium">{t(label)}</span>
      </>
    );
    // Every nav item opens its section's main page — clicking it from a nested
    // detail route always lands on the section root (no per-section special-casing).
    return (
      <div key={to} className="relative" data-tour-id={tourId}>
        {active && <NavPill layoutId="sidebar-pill" />}
        <Link to={to} className={linkClassName} onClick={scrollPageToTop}>
          {linkContent}
        </Link>
      </div>
    );
  };

  return (
    <aside className="app-sidebar glass-surface m-3 mr-0 flex w-60 flex-col rounded-2xl p-3">
      <div className="flex justify-end pb-2">
        <Button
          variant="ghost"
          size="sm"
          onClick={toggleSidebar}
          aria-label={t('nav.collapseSidebar')}
        >
          <PanelLeftClose size={16} />
        </Button>
      </div>
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

      <nav className="mt-auto flex flex-col gap-1 border-t border-foreground/[0.06] pb-3 pt-3">
        {PINNED_ITEMS.map(renderNavItem)}
      </nav>

      <div className="space-y-2 px-3 pb-3">
        {userName && (
          <div className="flex items-center gap-2.5 px-1">
            {avatar ? (
              <Image
                src={avatar}
                alt=""
                preview={false}
                rootClassName="h-8 w-8 shrink-0 rounded-full ring-1 ring-brand/20"
                className="h-8 w-8 object-cover"
              />
            ) : (
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-gradient-to-br from-brand-soft/25 to-brand/10 ring-1 ring-brand/20">
                <User size={15} className="text-brand-soft" />
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium leading-tight text-foreground/85">
                {userName}
              </div>
              <div className="text-xs leading-tight text-foreground/40">{getTimeGreeting()}</div>
            </div>
            <Link
              to={ROUTES.SETTINGS}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded-lg text-foreground/30 transition-colors hover:bg-foreground/[0.06] hover:text-foreground/60"
              onClick={scrollPageToTop}
            >
              <Settings size={12} />
            </Link>
          </div>
        )}
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
