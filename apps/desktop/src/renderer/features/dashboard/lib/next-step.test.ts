/**
 * The Dashboard next-step derivation, and the copy it renders.
 *
 * Two halves:
 *   1. `deriveNextStep` — pending beats everything, the suggestion order, and
 *      the "done" count being positional-free.
 *   2. Locale parity — every key the tile passes to `t()` resolves to a
 *      non-empty string in BOTH bundles. The JSON is imported directly rather
 *      than through `@ajh/translations`, which initializes i18next with
 *      `fallbackLng: 'en'` and would answer a missing `de` key with the
 *      English string (same reasoning as `features/support/support-data.test.ts`).
 */
import { describe, expect, it } from 'vitest';

import {
  deriveNextStep,
  NEXT_STEP_TEXT_KEYS,
  type NextStepSignals,
} from '@/features/dashboard/lib/next-step';

import de from '../../../../../../../packages/translations/src/locales/de/translation.json';
import en from '../../../../../../../packages/translations/src/locales/en/translation.json';

const BUNDLES: Record<string, unknown> = { en, de };

/**
 * Written out by hand, not derived from `NEXT_STEP_TEXT_KEYS` — a test that
 * loops over the constant it guards can only catch ADDITIONS, so deleting a
 * key from the export (and the copy with it) would otherwise stay green.
 */
const EXPECTED_KEYS = [
  'dashboard.nextStep.resume.title',
  'dashboard.nextStep.resume.description',
  'dashboard.nextStep.ai.title',
  'dashboard.nextStep.ai.description',
  'dashboard.nextStep.job.title',
  'dashboard.nextStep.job.description',
  'dashboard.nextStep.progress',
  'dashboard.nextStep.doneTitle',
  'dashboard.nextStep.unavailableTitle',
  'dashboard.nextStep.help',
];

/** Reason copy the AI step borrows from `AiSetupHint` instead of restating. */
const AI_REASON_KEYS = [
  'aiSetup.addApiKey',
  'aiSetup.selectModel',
  'aiSetup.installCli',
  'aiSetup.startOllama',
  'aiSetup.healthUnavailable',
];

/** Walks a dotted key through a bundle; `undefined` when a segment is missing. */
function lookup(bundle: unknown, key: string): unknown {
  return key
    .split('.')
    .reduce<unknown>(
      (node, segment) =>
        typeof node === 'object' && node !== null
          ? (node as Record<string, unknown>)[segment]
          : undefined,
      bundle
    );
}

/** `"<locale>: <key>"` for every key that is missing or blank. */
function unresolved(keys: readonly string[]): string[] {
  const missing: string[] = [];
  for (const [locale, bundle] of Object.entries(BUNDLES)) {
    for (const key of keys) {
      const value = lookup(bundle, key);
      if (typeof value !== 'string' || value.trim() === '') missing.push(`${locale}: ${key}`);
    }
  }
  return missing;
}

const signals = (patch: Partial<NextStepSignals> = {}): NextStepSignals => ({
  resume: true,
  ai: true,
  job: true,
  ...patch,
});

describe('deriveNextStep', () => {
  it('reports pending when any signal is still unanswered', () => {
    // Even with two steps already unmet — a cold boot must render nothing
    // rather than flash "add your résumé" at a user who has one.
    expect(deriveNextStep({ resume: 'pending', ai: false, job: false })).toEqual({
      kind: 'pending',
    });
    expect(deriveNextStep(signals({ ai: 'pending' }))).toEqual({ kind: 'pending' });
    expect(deriveNextStep(signals({ job: 'pending' }))).toEqual({ kind: 'pending' });
  });

  it('reports unavailable — not silence — when a signal query rejected', () => {
    // The signal that failed is `job`, which is met in every other respect
    // here: a hole anywhere makes both answers ("which step is first unmet"
    // and "how many are done") unknowable, so the tile owes the user a
    // neutral row instead of the permanent nothing a rejected query used to
    // produce.
    expect(deriveNextStep(signals({ job: 'unavailable' }))).toEqual({ kind: 'unavailable' });
    expect(deriveNextStep({ resume: 'unavailable', ai: false, job: false })).toEqual({
      kind: 'unavailable',
    });
  });

  it('still renders nothing while another signal is loading alongside a failed one', () => {
    // Pending is checked first: during a cold boot where one query has
    // already failed, showing nothing is better than a status row that
    // appears a moment before the rest of the page settles.
    expect(deriveNextStep({ resume: 'unavailable', ai: 'pending', job: true })).toEqual({
      kind: 'pending',
    });
  });

  it('suggests the first unmet step in the order résumé → ai → job', () => {
    // Each case leaves everything earlier in the order met, so the step it
    // returns can only come from the order itself.
    expect(deriveNextStep({ resume: false, ai: false, job: false })).toMatchObject({
      step: 'resume',
    });
    expect(deriveNextStep({ resume: true, ai: false, job: false })).toMatchObject({ step: 'ai' });
    expect(deriveNextStep({ resume: true, ai: true, job: false })).toMatchObject({ step: 'job' });
  });

  it('counts met steps regardless of position, not how far along the order it is', () => {
    // The trap this pins: a user who finished setup and then deleted their
    // résumé is back on step one but is NOT 0-of-3 done.
    expect(deriveNextStep({ resume: false, ai: true, job: true })).toEqual({
      kind: 'step',
      step: 'resume',
      done: 2,
      total: 3,
    });
    expect(deriveNextStep({ resume: false, ai: false, job: false })).toEqual({
      kind: 'step',
      step: 'resume',
      done: 0,
      total: 3,
    });
  });

  it('reports done once every step is met', () => {
    expect(deriveNextStep(signals())).toEqual({ kind: 'done' });
  });
});

describe('next-step copy / translations parity', () => {
  it('exports exactly the keys the tile renders', () => {
    expect([...NEXT_STEP_TEXT_KEYS]).toEqual(EXPECTED_KEYS);
  });

  it('resolves every next-step key to a non-empty string in en AND de', () => {
    expect(unresolved(NEXT_STEP_TEXT_KEYS)).toEqual([]);
  });

  it('resolves the AI reason copy the step borrows in en AND de', () => {
    expect(unresolved(AI_REASON_KEYS)).toEqual([]);
  });
});
