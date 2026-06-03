/**
 * Theme Engine
 * ─────────────────────────────────────────────────────────────────────────
 * Two orthogonal axes:
 *   • Color scheme — 'light' | 'dark' | 'system' (system follows the OS).
 *   • Accessibility modifiers — independent of scheme, each either forced on
 *     or "auto" (follows the matching OS preference):
 *       - reduceTransparency → solidifies glass (prefers-reduced-transparency)
 *       - contrast: 'more'   → stronger borders (prefers-contrast)
 *
 * Applied to <html> as data attributes the CSS token layer keys off:
 *   data-color-scheme="light|dark", data-reduce-transparency, data-contrast.
 * The CSS layer overrides only the design tokens per scheme, so one definition
 * drives every surface.
 * ─────────────────────────────────────────────────────────────────────────
 */

export type ColorScheme = 'light' | 'dark' | 'system';
export type ContrastPref = 'normal' | 'more';

export interface ThemePrefs {
  scheme: ColorScheme;
  /** Force reduced transparency on. When false, follows the OS preference. */
  reduceTransparency: boolean;
  /** 'more' forces high contrast on. 'normal' follows the OS preference. */
  contrast: ContrastPref;
}

const STORAGE_KEY = 'ajh-theme';

export const DEFAULT_THEME_PREFS: ThemePrefs = {
  scheme: 'system',
  reduceTransparency: false,
  contrast: 'normal',
};

const mq = (query: string): MediaQueryList | null =>
  typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia(query)
    : null;

const prefersDark = () => mq('(prefers-color-scheme: dark)')?.matches ?? true;
const prefersReducedTransparency = () =>
  mq('(prefers-reduced-transparency: reduce)')?.matches ?? false;
const prefersMoreContrast = () => mq('(prefers-contrast: more)')?.matches ?? false;

/** Resolve 'system' to a concrete scheme using the OS preference. */
export function getResolvedScheme(scheme: ColorScheme): 'light' | 'dark' {
  if (scheme === 'light' || scheme === 'dark') return scheme;
  return prefersDark() ? 'dark' : 'light';
}

let current: ThemePrefs = DEFAULT_THEME_PREFS;

/** Apply theme preferences to the document root and persist them. */
export function applyTheme(prefs: ThemePrefs): void {
  current = prefs;
  const root = document.documentElement;

  const resolved = getResolvedScheme(prefs.scheme);
  root.dataset.colorScheme = resolved;
  // Keep the legacy class in sync for any class-based styling.
  root.classList.toggle('dark', resolved === 'dark');
  root.classList.toggle('light', resolved === 'light');

  const reduce = prefs.reduceTransparency || prefersReducedTransparency();
  root.toggleAttribute('data-reduce-transparency', reduce);

  const moreContrast = prefs.contrast === 'more' || prefersMoreContrast();
  root.dataset.contrast = moreContrast ? 'more' : 'normal';

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    /* ignore */
  }
}

/** Read persisted prefs, migrating the legacy string format. */
export function getThemePrefs(): ThemePrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_THEME_PREFS;
    // Legacy single-string themes → modifier flags on the dark scheme.
    if (raw === 'default') return { scheme: 'dark', reduceTransparency: false, contrast: 'normal' };
    if (raw === 'reduced-glass')
      return { scheme: 'dark', reduceTransparency: true, contrast: 'normal' };
    if (raw === 'high-contrast')
      return { scheme: 'dark', reduceTransparency: false, contrast: 'more' };
    const parsed = JSON.parse(raw) as Partial<ThemePrefs>;
    if (parsed && typeof parsed === 'object' && typeof parsed.scheme === 'string') {
      return { ...DEFAULT_THEME_PREFS, ...parsed };
    }
  } catch {
    /* ignore */
  }
  return DEFAULT_THEME_PREFS;
}

let listenersBound = false;
/** Re-apply on OS changes so 'system' / 'auto' modifiers track live. */
function bindOsListeners(): void {
  if (listenersBound) return;
  listenersBound = true;
  const reapply = () => applyTheme(current);
  mq('(prefers-color-scheme: dark)')?.addEventListener('change', reapply);
  mq('(prefers-reduced-transparency: reduce)')?.addEventListener('change', reapply);
  mq('(prefers-contrast: more)')?.addEventListener('change', reapply);
}

/** Restore persisted prefs on boot and start tracking OS changes. */
export function restoreTheme(): void {
  applyTheme(getThemePrefs());
  bindOsListeners();
}
