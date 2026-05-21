import {
  Activity,
  Briefcase,
  FileText,
  Gauge,
  LayoutDashboard,
  type LucideIcon,
  Search,
  Settings,
  Sparkles,
  Wand2,
} from 'lucide-react';
import { motion } from 'motion/react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from '@tanstack/react-router';

import { ModalShell } from '@ajh/ui';

import { ROUTES } from '@/constants/routes';
import { cn } from '@/lib/cn';
import { useTranslation } from '@/lib/i18n';
import { stagger, transition } from '@/lib/motion';
import { useCommandPaletteShortcut } from '@/services';
import { useAppStore } from '@/store/app-store';

interface Action {
  id: string;
  label: string;
  group: 'navigate' | 'actions' | 'ai';
  icon: LucideIcon;
  run: () => void;
}

export function CommandPalette() {
  const open = useAppStore((s) => s.paletteOpen);
  const setOpen = useAppStore((s) => s.setPaletteOpen);
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Subscribe to main-process shortcut (Cmd/Ctrl+K from system tray / global shortcut)
  const handleOpen = useCallback(() => setOpen(true), [setOpen]);
  useCommandPaletteShortcut(handleOpen);

  // Global shortcut: Cmd/Ctrl+K from keyboard in renderer
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen(!open);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [open, setOpen]);

  // Reset state and focus input each time it opens
  useEffect(() => {
    if (open) {
      setQuery('');
      setActiveIndex(0);
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [open]);

  const actions: Action[] = useMemo(
    () => [
      {
        id: 'nav-dash',
        group: 'navigate',
        label: t('nav.dashboard'),
        icon: LayoutDashboard,
        run: () => navigate({ to: ROUTES.DASHBOARD }),
      },
      {
        id: 'nav-analyze',
        group: 'actions',
        label: t('nav.analyze'),
        icon: Gauge,
        run: () => navigate({ to: ROUTES.ANALYZE }),
      },
      {
        id: 'nav-generate',
        group: 'actions',
        label: t('nav.generate'),
        icon: Wand2,
        run: () => navigate({ to: ROUTES.GENERATE }),
      },
      {
        id: 'nav-jobs',
        group: 'navigate',
        label: t('nav.jobs'),
        icon: Briefcase,
        run: () => navigate({ to: ROUTES.JOBS }),
      },
      {
        id: 'nav-docs',
        group: 'navigate',
        label: t('nav.resumes'),
        icon: FileText,
        run: () => navigate({ to: ROUTES.RESUMES }),
      },
      {
        id: 'nav-ai',
        group: 'ai',
        label: t('nav.ai'),
        icon: Sparkles,
        run: () => navigate({ to: ROUTES.AI }),
      },
      {
        id: 'nav-monitoring',
        group: 'navigate',
        label: t('nav.monitoring'),
        icon: Activity,
        run: () => navigate({ to: ROUTES.MONITORING }),
      },
      {
        id: 'nav-settings',
        group: 'navigate',
        label: t('nav.settings'),
        icon: Settings,
        run: () => navigate({ to: ROUTES.SETTINGS }),
      },
    ],
    [navigate, t]
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? actions.filter((a) => a.label.toLowerCase().includes(q)) : actions;
  }, [query, actions]);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, filtered.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === 'Enter' && filtered[activeIndex]) {
      filtered[activeIndex].run();
      setOpen(false);
    }
  };

  return (
    <ModalShell open={open} onClose={() => setOpen(false)} maxWidth="max-w-[640px]" zIndex={600}>
      {/* Search input row */}
      <div
        role="combobox"
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-controls="palette-results"
        className="flex items-center gap-3 border-b border-white/5 px-5 py-4"
      >
        <Search size={16} className="shrink-0 text-foreground/50" aria-hidden="true" />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActiveIndex(0);
          }}
          onKeyDown={onKeyDown}
          placeholder={t('command.placeholder')}
          aria-label={t('command.placeholder')}
          aria-autocomplete="list"
          aria-activedescendant={
            filtered[activeIndex] ? `palette-item-${filtered[activeIndex].id}` : undefined
          }
          className="input-field flex-1 bg-transparent text-sm text-foreground placeholder:text-foreground/40"
        />
        <kbd className="shrink-0 rounded bg-white/5 px-1.5 py-0.5 text-[10px] text-foreground/50">
          ESC
        </kbd>
      </div>

      {/* Results */}
      <motion.div
        id="palette-results"
        role="listbox"
        aria-label="Command results"
        className="max-h-[50vh] overflow-y-auto p-2"
        variants={stagger.container}
        initial="hidden"
        animate="show"
        key={query}
      >
        {filtered.length === 0 ? (
          <div role="option" className="p-6 text-center text-sm text-foreground/50">
            {t('command.empty')}
          </div>
        ) : (
          filtered.map((a, i) => (
            <motion.button
              key={a.id}
              id={`palette-item-${a.id}`}
              role="option"
              aria-selected={i === activeIndex}
              variants={stagger.item}
              transition={transition.fast}
              onClick={() => {
                a.run();
                setOpen(false);
              }}
              onMouseEnter={() => setActiveIndex(i)}
              className={cn(
                'flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm transition-colors duration-100',
                i === activeIndex
                  ? 'bg-white/[0.07] text-foreground'
                  : 'text-foreground/70 hover:bg-white/[0.05] hover:text-foreground'
              )}
            >
              <a.icon size={15} className="shrink-0 text-foreground/50" aria-hidden="true" />
              <span className="flex-1">{a.label}</span>
              <span className="ml-auto text-[10px] uppercase tracking-wider text-foreground/30">
                {t(`command.groups.${a.group}`)}
              </span>
            </motion.button>
          ))
        )}
      </motion.div>
    </ModalShell>
  );
}
