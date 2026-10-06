/**
 * Shared mock factories + fixtures for the JobAdView Score-tab tests that run against the
 * REAL bundled en copy (`scoreLabels` / `scoreContext` / `scoreEgress` `.i18n.test.tsx`).
 *
 * Those tests deliberately do NOT mock `@ajh/translations` (same rationale as
 * match-band.i18n.test.tsx / trust-badge.test.tsx) — it resolves to the real
 * bundled en resources under vitest, so a mistyped/missing key, or an
 * accidental "ATS score" label, surfaces there instead of being hidden behind
 * `t: (k) => k`.
 *
 * `@/services`' `useJobAdTextMatchScore` IS stubbed — same pattern as
 * MatchScoresProvider.test.tsx — so no QueryClient/AppClient/IPC is needed.
 * It's a tracked `vi.fn` (not a plain arrow) so the resumeId-threading test
 * can assert on its call arguments.
 *
 * This module is what the `vi.mock` factories import, so it must NEVER import the
 * component under test (a value import would deadlock the factory) — the render
 * helpers live in `i18n-helpers.tsx`. The keystroke-storm guard (editing the
 * posting text must not re-fire the query) is covered separately, against the
 * same tracked-mock pattern, in `scoreTab.test.tsx`.
 */
import { type Mock, vi } from 'vitest';

import type { MatchScore } from '@ajh/shared';

import type { JobAdView } from '../JobAdView';

/** Mutable stub state the mocks below read. */
export const stub: {
  score: {
    data?: MatchScore;
    isLoading?: boolean;
    isError?: boolean;
    refetch?: () => void;
  };
  /** Selectable so the CLI-agent egress-disclosure tests can pick a CLI-agent
   *  provider; every other test leaves it at the default ('ollama', kind:
   *  local-server — no disclosure). */
  provider: string;
} = { score: {}, provider: 'ollama' };

/** Call in `beforeEach`. Resets EVERY test, not just the ones that assign it:
 *  the no-résumé / empty-posting / whitespace-only-posting cases never set
 *  `stub.score` — they pass because the component short-circuits on
 *  `!resumeId`/`!scoreText` before reading the score, but without this reset they'd
 *  silently inherit whatever the PREVIOUS test left behind. */
export function resetStub() {
  stub.score = {};
  stub.provider = 'ollama';
}

export const mockUseJobAdTextMatchScore = vi.fn(
  (_resumeId: string | null, _jobText: string, _enabled?: boolean) => stub.score
);

export const servicesModule: Record<string, unknown> = {
  useJobAdTextMatchScore: (...args: Parameters<typeof mockUseJobAdTextMatchScore>) =>
    mockUseJobAdTextMatchScore(...args),
};

// Self-contained store-driven pickers — never mounted on the Score tab (every
// test here starts on `source`, see makeProps), stubbed only so the module
// import itself stays cheap and hermetic.
export const modelSelectorModule = {
  ModelSelector: () => null,
  useSelectedProvider: () => stub.provider,
};

export const generateModule = { OUTPUT_LANGUAGES: [{ code: 'en', endonym: 'English' }] };

export const RESUME_ID = 'resume-1';
export const JOB_ID = 'job-1';

/** `combined === ats` whenever `scoreSource: 'keyword'` — the kernel's own
 *  invariant (`commands/match_resume/score.rs`: `combined` only diverges from `ats` when a
 *  semantic comparison actually ran, which sets `scoreSource: 'combined'`).
 *  A fixture that violates this (e.g. `ats: 60, combined: 55` under
 *  `'keyword'`) is one `match:text` can never actually produce. */
export function baseScore(overrides: Partial<MatchScore> = {}): MatchScore {
  return {
    resumeId: RESUME_ID,
    jobId: JOB_ID,
    ats: 60,
    semantic: 0,
    combined: 60,
    gaps: ['docker'],
    recommendations: [],
    scoreSource: 'keyword',
    ...overrides,
  };
}

/** Stubs the scoring hook with a measured, settled score. */
export function setScore(overrides: Partial<MatchScore> = {}) {
  stub.score = { data: baseScore(overrides), isLoading: false };
}

export function makeProps(overrides: Partial<Parameters<typeof JobAdView>[0]> = {}) {
  return {
    // Non-empty by default — the Score tab snapshots this text the instant it
    // opens, and an empty snapshot renders the "no posting" reason instead of
    // the stubbed score (see the "posting text is empty" test, which
    // overrides this back to '').
    jobDesc: 'A job posting with enough text to score against.',
    onJobDescChange: vi.fn() as Mock,
    summary: '',
    generating: false,
    error: null,
    onGenerateSummary: vi.fn() as Mock,
    language: 'en',
    onLanguageChange: vi.fn() as Mock,
    hasDesc: false, // defaults to the `source` tab — no summary toolbar to stub
    resumeId: RESUME_ID,
    ...overrides,
  };
}
