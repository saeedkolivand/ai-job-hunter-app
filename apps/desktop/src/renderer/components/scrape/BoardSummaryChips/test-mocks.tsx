/**
 * @ajh/translations + @ajh/ui mock factories for the BoardSummaryChips tests.
 *
 * @ajh/ui `Tag` is stubbed to surface its `color` prop as `data-color` so each
 * chip's tone is assertable; @ajh/translations is a readable identity mock.
 * `vi.mock` is hoisted per test file, so each file wires these in with
 * `vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock())`.
 * Kept apart from `test-support` (which imports the component) so the mock
 * factory never awaits a module that itself imports the mocked package.
 */

import React from 'react';

export function translationsMock() {
  return {
    useTranslation: () => ({
      t: (k: string, opts?: Record<string, unknown>) => {
        // `since` (the Track B1 history chips) is surfaced too, so a test can pin
        // the rendered relative time as an absolute string.
        if (opts && 'count' in opts && 'total' in opts)
          return `${k}:${String(opts.count)}/${String(opts.total)}`;
        if (opts && 'count' in opts && 'since' in opts)
          return `${k}:${String(opts.count)}:${String(opts.since)}`;
        if (opts && 'since' in opts) return `${k}:${String(opts.since)}`;
        if (opts && 'count' in opts) return `${k}:${String(opts.count)}`;
        if (opts && 'defaultValue' in opts) return `label(${String(opts.defaultValue)})`;
        return k;
      },
      // `note` chips resolve the country name via i18n.language + the real
      // regionName helper (not mocked), so the hook must expose i18n here.
      i18n: { language: 'en' },
    }),
  };
}

/** Exposes Tag color + className so tone AND wrap classes are assertable. */
export function uiMock() {
  return {
    Tag: ({
      color,
      className,
      children,
    }: {
      color?: string;
      className?: string;
      children?: React.ReactNode;
    }) => (
      <span data-testid="chip" data-color={color} className={className}>
        {children}
      </span>
    ),
    cn: (...args: unknown[]) => args.filter(Boolean).join(' '),
  };
}
