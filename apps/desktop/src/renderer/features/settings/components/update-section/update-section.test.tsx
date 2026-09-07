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

import i18n from '@ajh/translations';

import { resetUpdaterStatusForTests } from '@/services/use-updater';
import { createMockClient, withProviders } from '@/test-support';

import { UpdateSection } from './index';

const MANAGED_KEY = 'settings.update.managedByStore';

// `status` lives in module state inside use-updater (see its own test file for
// why), so it survives across `it()`s unless reset.
afterEach(() => {
  resetUpdaterStatusForTests();
});

function setup() {
  let handler: ((s: unknown) => void) | null = null;
  const client = createMockClient({
    'updater.onStatus': vi.fn((h: (s: unknown) => void) => {
      handler = h;
      return () => {};
    }),
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
    emit({ state: 'managed', by: 'store' });

    expect(screen.getByText(i18n.t(MANAGED_KEY))).toBeInTheDocument();
    // Nothing in this panel may offer a GitHub update path on a packaged
    // install — the shell refuses it, so an offer would only ever dead-end.
    expect(screen.queryByText(i18n.t('settings.update.checkNow'))).not.toBeInTheDocument();
    expect(
      screen.queryByText(i18n.t('settings.update.downloadFromGitHub'))
    ).not.toBeInTheDocument();
    expect(screen.queryByText(i18n.t('settings.update.upToDate'))).not.toBeInTheDocument();
  });
});
