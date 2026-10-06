/**
 * Harness for the JobsCommandBar suites.
 *
 * The real Zustand session store is used (no mock) so state flows naturally;
 * `@ajh/ui` is real so the chips' close buttons are the real controls.
 */
import type { ComponentProps } from 'react';
import { vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { useSessionStore } from '@/store/session-store';

import { JobsCommandBar } from './index';

// t() renders "key[param=value,…]" so both key and params are assertable.
// `i18n.language` is required by the real BoardSummaryChips.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, unknown>) =>
      p
        ? `${k}[${Object.entries(p)
            .map(([key, val]) => `${key}=${String(val)}`)
            .join(',')}]`
        : k,
    i18n: { language: 'en' },
  }),
}));

type BarProps = ComponentProps<typeof JobsCommandBar>;

const baseProps: BarProps = {
  shownCount: 3,
  totalCount: 5,
  scraping: false,
  scrapeProgress: null,
  canClear: true,
  onClear: vi.fn(),
  onScrape: vi.fn(),
  onCancelScrape: vi.fn(),
  boardSummaries: [],
  failureNote: null,
  // Most tests predate the visibility gate and assume the work-type control
  // renders; default it "on" so they keep testing what they were written to
  // test. The gate itself has its own describe block, which overrides this.
  hasDeclaredWorkType: true,
  searchState: 'idle',
  onSubmitSearch: vi.fn(),
};

export function renderBar(overrides: Partial<BarProps> = {}) {
  return render(<JobsCommandBar {...baseProps} {...overrides} />);
}

/** Re-render with `overrides` applied over the base props (same DOM tree). */
export function rerenderBar(view: ReturnType<typeof renderBar>, overrides: Partial<BarProps>) {
  view.rerender(<JobsCommandBar {...baseProps} {...overrides} />);
}

export function setJobs(
  patch: Parameters<ReturnType<typeof useSessionStore.getState>['setJobs']>[0]
) {
  act(() => {
    useSessionStore.getState().setJobs(patch);
  });
}

/** Call from `beforeEach`. */
export function resetBar() {
  setJobs({ filter: '', sortBy: 'newest', viewMode: 'list', hideAgency: false, workTypes: [] });
}
