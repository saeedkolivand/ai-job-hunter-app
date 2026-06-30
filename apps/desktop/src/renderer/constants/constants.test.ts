import { describe, expect, it } from 'vitest';

import { AUTH_BOARD_IDS, AUTH_BOARDS } from './auth';
import { LOCALE_CODES, LOCALES } from './locales';
import { ROUTES } from './routes';
import { SUPPORT_TABS } from './support';
import { TAB_GROUPS, TAB_IDS, TABS } from './tabs';

describe('settings tabs constants', () => {
  it('defines a tab for every TAB_ID', () => {
    const tabIds = TABS.map((t) => t.id);
    for (const id of Object.values(TAB_IDS)) {
      expect(tabIds).toContain(id);
    }
  });

  it('every tab has a label and icon', () => {
    for (const tab of TABS) {
      expect(tab.label).toBeTruthy();
      expect(tab.icon).toBeTruthy();
    }
  });

  it('groups reference known tab ids', () => {
    const known = new Set(Object.values(TAB_IDS));
    for (const group of TAB_GROUPS) {
      expect(group.label).toBeTruthy();
      for (const id of group.tabs) expect(known.has(id)).toBe(true);
    }
  });
});

describe('auth + locale + route constants', () => {
  it('lists auth boards keyed consistently', () => {
    const ids = AUTH_BOARDS.map((b) => b.id);
    for (const id of Object.values(AUTH_BOARD_IDS)) {
      expect(ids).toContain(id);
    }
  });

  it('exposes the supported locales', () => {
    expect(Object.values(LOCALE_CODES)).toContain('en');
    expect(LOCALES.length).toBeGreaterThan(0);
    for (const l of LOCALES) expect(l).toHaveProperty('code');
  });

  it('exposes non-empty route paths', () => {
    expect(Object.keys(ROUTES).length).toBeGreaterThan(0);
    for (const path of Object.values(ROUTES)) {
      expect(typeof path).toBe('string');
    }
  });

  it('exposes support tabs', () => {
    expect(SUPPORT_TABS.length).toBeGreaterThan(0);
  });
});
