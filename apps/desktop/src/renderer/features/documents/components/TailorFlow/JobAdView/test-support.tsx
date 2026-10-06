/**
 * Shared stubs + props builder for the JobAdView tests that echo raw i18n keys
 * (`JobAdView.test.tsx`, `scoreTab.test.tsx`). `vi.mock` is hoisted per test
 * file, so each file keeps one-line `vi.mock(..., async () => (await
 * import('./test-support')).x)` calls and this module owns what they return.
 *
 * Strategy:
 *  - `@ajh/ui` primitives are NOT stubbed — real primitives catch future API
 *    changes early.
 *  - `useJobAdTextMatchScore` is a tracked `vi.fn` (not a plain arrow) so no
 *    QueryClient/AppClient/IPC is needed while tests can still assert on ITS
 *    call arguments — the component-level guard that a keystroke never flips
 *    `enabled` to `true`. The hook is called unconditionally on every render
 *    (Rules of Hooks), so SOME `@/services` binding must exist.
 *    `mockUseJobAdTextMatchScore` (the `mock`-prefixed name) is Vitest's
 *    documented exception to the "no out-of-scope refs in a hoisted factory"
 *    rule — see MatchScoresProvider.test.tsx for the same pattern.
 *  - The Score tab's real vs. "not scored" render logic is covered separately
 *    against REAL translated copy in the `*.i18n.test.tsx` files.
 */
import type React from 'react';
import { type Mock, vi } from 'vitest';

import type { JobAdView } from '../JobAdView';

// className is forwarded so the containment test can assert it — the real
// component applies `className` to its own root div.
export const modelSelectorModule = {
  ModelSelector: ({ className }: { className?: string }) => (
    <div data-testid="model-selector-stub" className={className} />
  ),
  // Score-tab CLI-agent egress disclosure reads this — 'ollama' (kind:
  // local-server) keeps every assertion here unaffected; the real-copy
  // assertion for the disclosure itself lives in the i18n tests.
  useSelectedProvider: () => 'ollama',
};

export const externalLinkModule = {
  ExternalLink: ({
    href,
    children,
    ...rest
  }: { href: string; children: React.ReactNode } & React.HTMLAttributes<HTMLAnchorElement>) => (
    <a href={href} {...rest}>
      {children}
    </a>
  ),
};

// Only the shape matters; stub to a minimal list.
export const generateModule = { OUTPUT_LANGUAGES: [{ code: 'en', endonym: 'English' }] };

export const mockUseJobAdTextMatchScore = vi.fn(
  (_resumeId: string | null, _jobText: string, _enabled?: boolean) => ({
    data: undefined,
    isLoading: false,
    isError: false,
    refetch: vi.fn() as Mock,
  })
);

export const servicesModule: Record<string, unknown> = {
  useJobAdTextMatchScore: (...args: Parameters<typeof mockUseJobAdTextMatchScore>) =>
    mockUseJobAdTextMatchScore(...args),
};

export function makeProps(overrides: Partial<Parameters<typeof JobAdView>[0]> = {}) {
  return {
    jobDesc: 'Full job description with enough text.',
    onJobDescChange: vi.fn() as Mock,
    summary: '',
    generating: false,
    error: null,
    onGenerateSummary: vi.fn() as Mock,
    language: 'en',
    onLanguageChange: vi.fn() as Mock,
    hasDesc: true,
    fetchingDesc: false,
    jobUrl: undefined,
    ...overrides,
  };
}
