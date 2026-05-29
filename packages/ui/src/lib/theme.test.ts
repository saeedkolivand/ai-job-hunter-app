import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { applyTheme, getActiveTheme, restoreTheme, THEMES } from './theme';

describe('theme engine', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.removeAttribute('style');
  });

  afterEach(() => {
    localStorage.clear();
  });

  it('exposes the three built-in themes', () => {
    expect(THEMES.map((t) => t.id)).toEqual(['default', 'reduced-glass', 'high-contrast']);
  });

  it('applies a theme by setting CSS vars + dataset + storage', () => {
    applyTheme('reduced-glass');
    expect(document.documentElement.dataset.theme).toBe('reduced-glass');
    expect(document.documentElement.style.getPropertyValue('--blur-sm')).toBe('4px');
    expect(localStorage.getItem('ajh-theme')).toBe('reduced-glass');
  });

  it('clears previous theme vars when switching themes', () => {
    applyTheme('reduced-glass');
    applyTheme('high-contrast');
    expect(document.documentElement.style.getPropertyValue('--blur-sm')).toBe('');
    expect(document.documentElement.style.getPropertyValue('--border-faint')).not.toBe('');
  });

  it('getActiveTheme reflects the applied theme and defaults otherwise', () => {
    expect(getActiveTheme()).toBe('default');
    applyTheme('high-contrast');
    expect(getActiveTheme()).toBe('high-contrast');
  });

  it('restoreTheme reapplies the persisted theme', () => {
    localStorage.setItem('ajh-theme', 'reduced-glass');
    restoreTheme();
    expect(getActiveTheme()).toBe('reduced-glass');
  });

  it('restoreTheme is a no-op when nothing is persisted', () => {
    restoreTheme();
    expect(getActiveTheme()).toBe('default');
  });
});
