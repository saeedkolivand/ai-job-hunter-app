import { Command } from 'lucide-react';
import { useCallback } from 'react';
import { useRouter } from '@tanstack/react-router';
import { platform } from '@tauri-apps/plugin-os';

import { useTranslation } from '@ajh/translations';
import { ModalShell, SectionLabel } from '@ajh/ui';

import { type AppRoute, useKeyboardShortcuts } from '@/hooks/use-keyboard-shortcuts';
import { useUiStore } from '@/store/ui-store';

const MOD = platform() === 'macos' ? '⌘' : 'Ctrl';

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
      header={
        <div className="flex items-center gap-2 border-b border-white/5 px-6 py-4">
          <Command size={15} className="text-brand-soft" />
          <span className="text-sm font-medium text-foreground/90">{t('shortcuts.title')}</span>
        </div>
      }
      footer={
        <div className="border-t border-white/5 px-6 py-3 text-center text-[11px] text-foreground/35">
          {t('shortcuts.hint')}
        </div>
      }
    >
      <div className="grid grid-cols-1 gap-x-8 gap-y-4 px-6 py-5 @sm:grid-cols-2">
        <div className="space-y-2.5">
          <SectionLabel>{t('shortcuts.navigate')}</SectionLabel>
          <ShortcutList rows={NAVIGATE} t={t} />
        </div>
        <div className="space-y-2.5">
          <SectionLabel>{t('shortcuts.actions')}</SectionLabel>
          <ShortcutList rows={ACTIONS} t={t} />
        </div>
      </div>
    </ModalShell>
  );
}
