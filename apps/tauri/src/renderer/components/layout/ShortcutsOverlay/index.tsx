import { Command } from 'lucide-react';
import { useCallback } from 'react';
import { useRouter } from '@tanstack/react-router';

import { ModalShell, SectionLabel } from '@ajh/ui';

import { type AppRoute, useKeyboardShortcuts } from '@/hooks/use-keyboard-shortcuts';
import { useTranslation } from '@/lib/i18n';
import { useUiStore } from '@/store/ui-store';

const IS_MAC = typeof navigator !== 'undefined' && navigator.platform.toLowerCase().includes('mac');
const MOD = IS_MAC ? '⌘' : 'Ctrl';

interface Row {
  keys: string[];
  labelKey: string;
}

const NAVIGATE: Row[] = [
  { keys: ['g', 'd'], labelKey: 'nav.dashboard' },
  { keys: ['g', 'a'], labelKey: 'nav.analyze' },
  { keys: ['g', 'g'], labelKey: 'nav.generate' },
  { keys: ['g', 'j'], labelKey: 'nav.jobs' },
  { keys: ['g', 'p'], labelKey: 'nav.autopilot' },
  { keys: ['g', 'r'], labelKey: 'nav.documents' },
  { keys: ['g', 'm'], labelKey: 'nav.monitoring' },
  { keys: ['g', 's'], labelKey: 'nav.settings' },
];

const ACTIONS: Row[] = [
  { keys: [MOD, 'K'], labelKey: 'nav.search' },
  { keys: [MOD, ','], labelKey: 'nav.settings' },
  { keys: ['?'], labelKey: 'shortcuts.help' },
];

function Kbd({ children }: { children: string }) {
  return (
    <kbd className="inline-flex h-6 min-w-[1.5rem] items-center justify-center rounded-md border border-white/10 bg-white/[0.06] px-1.5 font-mono text-[11px] text-foreground/70">
      {children}
    </kbd>
  );
}

function ShortcutList({ rows, t }: { rows: Row[]; t: (k: string) => string }) {
  return (
    <div className="space-y-1.5">
      {rows.map((row) => (
        <div
          key={row.labelKey + row.keys.join()}
          className="flex items-center justify-between gap-4"
        >
          <span className="text-xs text-foreground/65">{t(row.labelKey)}</span>
          <span className="flex shrink-0 items-center gap-1">
            {row.keys.map((k, i) => (
              <Kbd key={i}>{k}</Kbd>
            ))}
          </span>
        </div>
      ))}
    </div>
  );
}

/**
 * Mounts the global keyboard-shortcut handler and the `?` cheat-sheet. Rendered
 * once in the root layout.
 */
export function ShortcutsOverlay() {
  const router = useRouter();
  const { t } = useTranslation();
  // Open state is lifted to the UI store so the native menu's "Keyboard
  // Shortcuts" action can open it; the `?` key still toggles it locally.
  const open = useUiStore((s) => s.shortcutsOpen);
  const setOpen = useUiStore((s) => s.setShortcutsOpen);

  const onNavigate = useCallback((to: AppRoute) => void router.navigate({ to }), [router]);
  const onToggleHelp = useCallback(() => setOpen(!open), [open, setOpen]);
  useKeyboardShortcuts({ onNavigate, onToggleHelp });

  return (
    <ModalShell
      open={open}
      onClose={() => setOpen(false)}
      ariaLabel={t('shortcuts.title')}
      maxWidth="max-w-lg"
    >
      <div className="flex items-center gap-2 border-b border-white/5 px-6 py-4">
        <Command size={15} className="text-brand-soft" />
        <span className="text-sm font-medium text-foreground/90">{t('shortcuts.title')}</span>
      </div>
      <div className="grid grid-cols-2 gap-x-8 gap-y-4 px-6 py-5">
        <div className="space-y-2.5">
          <SectionLabel>{t('shortcuts.navigate')}</SectionLabel>
          <ShortcutList rows={NAVIGATE} t={t} />
        </div>
        <div className="space-y-2.5">
          <SectionLabel>{t('shortcuts.actions')}</SectionLabel>
          <ShortcutList rows={ACTIONS} t={t} />
        </div>
      </div>
      <div className="border-t border-white/5 px-6 py-3 text-center text-[11px] text-foreground/35">
        {t('shortcuts.hint')}
      </div>
    </ModalShell>
  );
}
