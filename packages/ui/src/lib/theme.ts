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

import { lightenHex, readableForeground } from './color';

export type ColorScheme = 'light' | 'dark' | 'system';
export type ContrastPref = 'normal' | 'more';
/** UI text size — scales the rem root so every rem-based size grows together. */
export type TextScale = 'small' | 'default' | 'large';
/**
 * Accent source: 'default' keeps the shipped per-scheme violet (no override);
 * 'system' uses the OS accent (resolved to a hex by the settings layer via the
 * native read); 'custom' uses a user-picked hex. The last two both resolve to
 * `accentColor`, which the applier writes to --color-brand on :root.
 */
export type AccentSource = 'default' | 'system' | 'custom';

export interface ThemePrefs {
  scheme: ColorScheme;
  /** Force reduced transparency on. When false, follows the OS preference. */
  reduceTransparency: boolean;
  /** 'more' forces high contrast on. 'normal' follows the OS preference. */
  contrast: ContrastPref;
  /** UI text size; sets the rem root (small 15px / default 16px / large 18px). */
  textScale: TextScale;
  /** Accent source. 'default' clears any accent override. */
  accentSource: AccentSource;
  /** Resolved hex accent for 'system'/'custom' (e.g. '#a855f7'). */
  accentColor?: string;
}

const STORAGE_KEY = 'ajh-theme';

export const DEFAULT_THEME_PREFS: ThemePrefs = {
  scheme: 'system',
  reduceTransparency: false,
  contrast: 'normal',
  textScale: 'default',
  accentSource: 'default',
};

/** Accent-derived CSS vars the applier owns on :root (cleared for 'default'). */
const ACCENT_VARS = ['--color-brand', '--color-brand-soft', '--color-action-foreground'] as const;

/**
 * Write (or clear) the runtime accent override on :root. brand-dim, the focus
 * ring, action-primary, and the brand glows all derive from --color-brand via
 * CSS, so only brand + its lighter step + the auto-contrast label need setting.
 * Invalid/absent colors fall back to Default (clears the overrides).
 */
function applyAccent(root: HTMLElement, prefs: ThemePrefs, scheme: 'light' | 'dark'): void {
  const color = prefs.accentSource === 'default' ? undefined : prefs.accentColor;
  // Dark canvas needs a bigger lift for the soft step than the light canvas.
  const softAmount = scheme === 'dark' ? 0.28 : 0.16;
  const soft = color ? lightenHex(color, softAmount) : null;
  const foreground = color ? readableForeground(color) : null;

  if (!color || !soft || !foreground) {
    for (const v of ACCENT_VARS) root.style.removeProperty(v);
    return;
  }
  root.style.setProperty('--color-brand', color);
  root.style.setProperty('--color-brand-soft', soft);
  root.style.setProperty('--color-action-foreground', foreground);
}

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

  // Scales the rem root; CSS keys font-size off data-text-scale (small/large).
  root.dataset.textScale = prefs.textScale;

  // Accent override (single source of truth: --color-brand). Applied after the
  // scheme so the soft-step lift matches the resolved light/dark canvas.
  applyAccent(root, prefs, resolved);

  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    /* ignore */
  }
}

/**
 * Apply prefs with a smooth full-page crossfade (View Transition API) when it's
 * supported, so switching scheme — especially dark→light — eases instead of
 * snapping (no "flashbang"). Falls back to an instant apply where the API is
 * unavailable or the user prefers reduced motion. Use for user-initiated
 * changes; boot and OS-driven re-applies stay instant.
 */
export function applyThemeAnimated(prefs: ThemePrefs): void {
  const doc = document as Document & {
    startViewTransition?: (callback: () => void) => unknown;
  };
  const reduceMotion = mq('(prefers-reduced-motion: reduce)')?.matches ?? false;
  if (typeof doc.startViewTransition === 'function' && !reduceMotion) {
    doc.startViewTransition(() => applyTheme(prefs));
  } else {
    applyTheme(prefs);
  }
}

/** Read persisted prefs, migrating the legacy string format. */
export function getThemePrefs(): ThemePrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_THEME_PREFS;
    // Legacy single-string themes → modifier flags on the dark scheme.
    if (raw === 'default') return { ...DEFAULT_THEME_PREFS, scheme: 'dark' };
    if (raw === 'reduced-glass')
      return { ...DEFAULT_THEME_PREFS, scheme: 'dark', reduceTransparency: true };
    if (raw === 'high-contrast')
      return { ...DEFAULT_THEME_PREFS, scheme: 'dark', contrast: 'more' };
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
