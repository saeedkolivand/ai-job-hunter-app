/**
 * `@ajh/translations` resolves to real source in vitest and initializes with
 * the bundled en/de resources as an import side effect, so `t()` returns real
 * copy here. Assertions therefore go through `i18n.t(...)` rather than
 * hardcoded English — with one `t(key) !== key` guard so a deleted key can't
 * make the DOM query pass vacuously (component and test would both fall back
 * to the raw key string).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import i18n from '@ajh/translations';

import { resetUpdaterStatusForTests } from '@/services/use-updater';
import { createMockClient, withProviders } from '@/test-support';

import { UpdateSection } from './index';

const MANAGED_KEY = 'settings.update.managedByStore';
const MANAGED_KEY_BY_FLAVOUR = {
  msstore: 'settings.update.managedByStore',
  flatpak: 'settings.update.managedByFlatpak',
  snap: 'settings.update.managedBySnap',
} as const;

// `status` lives in module state inside use-updater (see its own test file for
// why), so it survives across `it()`s unless reset.
afterEach(() => {
  resetUpdaterStatusForTests();
});

function setup(changelog: unknown = { releases: [] }) {
  let handler: ((s: unknown) => void) | null = null;
  const client = createMockClient({
    'updater.onStatus': vi.fn((h: (s: unknown) => void) => {
      handler = h;
      return () => {};
    }),
    'updater.changelog': vi.fn().mockResolvedValue(changelog),
    'system.getVersion': vi.fn().mockResolvedValue('0.1.0'),
  });
  const Wrapper = withProviders(client);
  render(
    <Wrapper>
      <UpdateSection />
    </Wrapper>
  );
  return { emit: (s: unknown) => act(() => handler?.(s)) };
}

describe('UpdateSection', () => {
  it('offers a manual check on a normal install', () => {
    const { emit } = setup();
    emit({ state: 'not-available' });

    expect(screen.getByText(i18n.t('settings.update.checkNow'))).toBeInTheDocument();
    expect(screen.getByText(i18n.t('settings.update.upToDate'))).toBeInTheDocument();
    expect(screen.queryByText(i18n.t(MANAGED_KEY))).not.toBeInTheDocument();
  });

  it('hands updates to the Store and hides the check button on a Store build', () => {
    expect(i18n.t(MANAGED_KEY)).not.toBe(MANAGED_KEY);
    const { emit } = setup();
    emit({ state: 'managed', by: 'msstore' });

    // A live region, because this line replaces the control the user activated.
    expect(screen.getByRole('status')).toHaveTextContent(i18n.t(MANAGED_KEY));
    // Nothing in this panel may offer a GitHub update path on a packaged
    // install — the shell refuses it, so an offer would only ever dead-end.
    expect(screen.queryByText(i18n.t('settings.update.checkNow'))).not.toBeInTheDocument();
    expect(screen.queryByText(i18n.t('settings.update.upToDate'))).not.toBeInTheDocument();
  });

  // Mutation-visible: hardcode the Store copy for every flavour and this
  // fails for the Flatpak/Snap cases — the whole reason this fix exists is
  // that a Flatpak/Snap user must never be told they installed from the
  // Microsoft Store.
  it.each(['msstore', 'flatpak', 'snap'] as const)(
    "renders the '%s' flavour's own copy, not another flavour's",
    (by) => {
      const key = MANAGED_KEY_BY_FLAVOUR[by];
      expect(i18n.t(key)).not.toBe(key);
      const { emit } = setup();
      emit({ state: 'managed', by });

      expect(screen.getByRole('status')).toHaveTextContent(i18n.t(key));
      for (const other of ['msstore', 'flatpak', 'snap'] as const) {
        if (other === by) continue;
        expect(screen.queryByText(i18n.t(MANAGED_KEY_BY_FLAVOUR[other]))).not.toBeInTheDocument();
      }
    }
  );

  // The changelog is collapsed by default, so its GitHub fallback is only
  // reachable after expanding — which is exactly why the pair below asserts
  // BOTH directions: an unconditional button passes the "absent" half only
  // because nothing rendered it yet.
  describe('the changelog fallback when release history fails to load', () => {
    it('offers the GitHub download on a normal install', async () => {
      const user = userEvent.setup();
      const { emit } = setup({ error: 'boom' });
      emit({ state: 'not-available' });

      await user.click(screen.getByText(i18n.t('settings.update.viewChangelog')));

      expect(await screen.findByText(i18n.t('settings.update.changelogError'))).toBeInTheDocument();
      expect(screen.getByText(i18n.t('settings.update.downloadFromGitHub'))).toBeInTheDocument();
    });

    it('does not offer it on a Store build', async () => {
      const user = userEvent.setup();
      const { emit } = setup({ error: 'boom' });
      emit({ state: 'managed', by: 'msstore' });

      await user.click(screen.getByText(i18n.t('settings.update.viewChangelog')));

      expect(await screen.findByText(i18n.t('settings.update.changelogError'))).toBeInTheDocument();
      expect(
        screen.queryByText(i18n.t('settings.update.downloadFromGitHub'))
      ).not.toBeInTheDocument();
    });
  });
});
