import { vi } from 'vitest';

import type { ThemePrefs } from '../theme';

// jsdom has no matchMedia — stub it so OS-preference probing is deterministic.
export function stubMatchMedia(matches: Record<string, boolean> = {}) {
  vi.stubGlobal(
    'matchMedia',
    (query: string) =>
      ({
        matches: matches[query] ?? false,
        media: query,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
        onchange: null,
      }) as unknown as MediaQueryList
  );
}

export const basePrefs: ThemePrefs = {
  scheme: 'dark',
  reduceTransparency: false,
  contrast: 'normal',
  textScale: 'default',
  accentSource: 'default',
};

export const cssVar = (name: string) => document.documentElement.style.getPropertyValue(name);

/** `beforeEach` body: clean storage + root element, no OS preferences. */
export function resetThemeEnv() {
  localStorage.clear();
  const root = document.documentElement;
  for (const attr of [
    'data-color-scheme',
    'data-reduce-transparency',
    'data-contrast',
    'data-text-scale',
  ]) {
    root.removeAttribute(attr);
  }
  root.className = '';
  root.style.cssText = '';
  stubMatchMedia();
}

/** `afterEach` body. */
export function restoreThemeEnv() {
  localStorage.clear();
  vi.unstubAllGlobals();
}

export function setStartViewTransition(value: unknown) {
  Object.defineProperty(document, 'startViewTransition', {
    value,
    configurable: true,
    writable: true,
  });
}
