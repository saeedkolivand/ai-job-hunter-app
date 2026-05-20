/**
 * Theme Engine
 * ─────────────────────────────────────────────────────────────────────────
 * Manages the active visual theme via CSS custom property overrides on
 * the document root. New themes only need to declare the properties they
 * change — the rest fall back to the defaults in globals.css.
 *
 * Adding a theme:
 *   1. Add an entry to THEMES below.
 *   2. Call applyTheme('your-theme-id') from a settings toggle.
 *   3. Persist the selected theme in preferences-store.
 *
 * Accessibility:
 *   The 'reduced-glass' theme lowers backdrop-filter intensity for users
 *   with vestibular disorders or low-performance hardware.
 * ─────────────────────────────────────────────────────────────────────────
 */

export type ThemeId = 'default' | 'reduced-glass' | 'high-contrast';

interface Theme {
  id: ThemeId;
  label: string;
  description: string;
  /** CSS custom property overrides applied to :root */
  vars: Partial<Record<string, string>>;
}

export const THEMES: Theme[] = [
  {
    id: 'default',
    label: 'Default',
    description: 'Full glassmorphism — cinematic purple-violet.',
    vars: {},
  },
  {
    id: 'reduced-glass',
    label: 'Reduced Glass',
    description: 'Lower blur and opacity for better performance or motion sensitivity.',
    vars: {
      '--blur-sm': '4px',
      '--blur-md': '8px',
      '--blur-lg': '10px',
      '--blur-xl': '12px',
      '--blur-2xl': '16px',
      '--blur-3xl': '24px',
    },
  },
  {
    id: 'high-contrast',
    label: 'High Contrast',
    description: 'Stronger borders and reduced opacity for accessibility.',
    vars: {
      '--border-faint': 'rgba(255, 255, 255, 0.12)',
      '--border-dim': 'rgba(255, 255, 255, 0.18)',
      '--border-soft': 'rgba(255, 255, 255, 0.22)',
      '--border-mid': 'rgba(255, 255, 255, 0.28)',
      '--border-clear': 'rgba(255, 255, 255, 0.35)',
    },
  },
];

const STORAGE_KEY = 'ajh-theme';

/** Apply a theme by writing its CSS vars to :root. */
export function applyTheme(id: ThemeId): void {
  const theme = THEMES.find((t) => t.id === id) as (typeof THEMES)[number];
  const root = document.documentElement;

  // Clear any previous theme vars first
  for (const t of THEMES) {
    for (const key of Object.keys(t.vars)) {
      root.style.removeProperty(key);
    }
  }

  // Apply the new theme
  for (const [key, value] of Object.entries(theme.vars)) {
    if (value) root.style.setProperty(key, value);
  }

  root.dataset.theme = id;
  try {
    localStorage.setItem(STORAGE_KEY, id);
  } catch {
    /* ignore */
  }
}

/** Restore the persisted theme on boot. */
export function restoreTheme(): void {
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as ThemeId | null;
    if (saved) applyTheme(saved);
  } catch {
    /* ignore */
  }
}

/** Read the currently active theme id. */
export function getActiveTheme(): ThemeId {
  return (document.documentElement.dataset.theme as ThemeId) ?? 'default';
}
