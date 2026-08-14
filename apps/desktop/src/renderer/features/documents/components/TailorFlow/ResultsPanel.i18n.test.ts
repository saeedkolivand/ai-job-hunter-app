/**
 * N1 regression: a mocked `t` that echoes the key verbatim is structurally
 * blind to a MISSING key — `ResultsPanel.test.tsx`'s mock passed even while
 * `pipeline.status.done` rendered the raw dotted key in production, because
 * the mock has no notion of "this key doesn't exist in the real catalog".
 *
 * CR-11: uses the REAL @ajh/translations instance (not a deep import of the
 * raw locale JSON, not ResultsPanel.test.tsx's identity mock) — same pattern
 * as this repo's other `*.i18n.test.ts` files (e.g.
 * LocationFilterNote.i18n.test.ts, AutopilotCard.i18n.test.ts):
 * `i18n.exists(key, { lng, fallbackLng: false })` + `i18n.getFixedT(lng)`.
 * `fallbackLng: false` disables @ajh/translations' `fallbackLng: 'en'`
 * default for this one check, so a key missing in de fails the de case
 * instead of silently resolving via English — per-locale separation is the
 * whole point, not just a shared "resolves at all" check. Each case below is
 * a (locale, key) pair, not just a key, so a de-only gap is reported on its
 * own case instead of hiding behind a passing en case for the same key.
 *
 * Scope (CR-3 correction of the original claim, which said "every
 * dynamically-constructed key path" but only swept `pipeline.status.*`,
 * `pipeline.step.*`, and `autopilot.apply.target.*`): every key
 * `ResultsPanel`/`GeneratingPanel` can reach at runtime that is either (a)
 * built from a CLOSED, enumerable input set — a union type, an exported
 * record, or a shared helper's own key table — or (b) a literal string key
 * this PR added or renamed, where the risk is a typo between the call site
 * and the catalog rather than a missing case. `pipeline.stopped.*` is (a):
 * `stoppedSuffix()` folds ANY backend `StoppedReason` onto
 * `STOPPED_SUFFIX`'s values or `UNKNOWN_STOPPED_SUFFIX`, so iterating that
 * exact set (not a hand-copied list) is what makes this sweep, not the
 * component's per-case `t()` mock, the source of truth.
 */
import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

import { STOPPED_SUFFIX, UNKNOWN_STOPPED_SUFFIX } from '@/lib/stopped-reason';

import { PIPELINE_STEP_KEYS } from './lib/pipeline-steps';
import { PIPELINE_STATUS_KEY } from './ResultsPanel';

const LOCALES = ['en', 'de'] as const;
type Locale = (typeof LOCALES)[number];

/**
 * CR-5: an existence check alone passes for a key that resolves to an empty
 * string — assert real, non-empty copy for the given locale, not just
 * presence.
 */
function expectRealCopy(lng: Locale, path: string) {
  expect(i18n.exists(path, { lng, fallbackLng: false }), `${lng}:${path}`).toBe(true);
  const out = i18n.getFixedT(lng)(path);
  expect(out, `${lng}:${path}`).not.toBe(path);
  expect(out.trim().length, `${lng}:${path}`).toBeGreaterThan(0);
}

describe('ResultsPanel i18n — every reachable key resolves to real content', () => {
  // N1: `pipeline.status.*` — the Tag's key, driven by TailorRunState via
  // PIPELINE_STATUS_KEY. This is the exact bug: `pipeline.status.done` and
  // `pipeline.status.error` do not exist — the vocabulary is
  // running/completed/needsReview/failed/cancelled.
  const statusCases = LOCALES.flatMap((lng) =>
    Object.entries(PIPELINE_STATUS_KEY).map(
      ([runState, statusKey]) => [lng, runState, statusKey] as const
    )
  );
  it.each(statusCases)('%s runState=%s -> pipeline.status.%s', (lng, _runState, statusKey) => {
    expectRealCopy(lng, `pipeline.status.${statusKey}`);
  });

  // CR-3: `pipeline.stopped.${suffix}` (ResultsPanel's stopped-reason line) —
  // the reachable set is every `StoppedSuffix` VALUE `stoppedSuffix()` can
  // produce, plus its own unknown-reason fallback. Iterated from the shared
  // helper's own tables, not a hand-copied list, so a variant added there is
  // covered here automatically.
  const stoppedSuffixes = [...new Set(Object.values(STOPPED_SUFFIX)), UNKNOWN_STOPPED_SUFFIX];
  const stoppedCases = LOCALES.flatMap((lng) =>
    stoppedSuffixes.map((suffix) => [lng, suffix] as const)
  );
  it.each(stoppedCases)('%s pipeline.stopped.%s', (lng, suffix) => {
    expectRealCopy(lng, `pipeline.stopped.${suffix}`);
  });

  // H8: pipeline.step.state.{done,active,pending} — the per-row sr-only word.
  const stepStateCases = LOCALES.flatMap((lng) =>
    (['done', 'active', 'pending'] as const).map((state) => [lng, state] as const)
  );
  it.each(stepStateCases)('%s pipeline.step.state.%s', (lng, state) => {
    expectRealCopy(lng, `pipeline.step.state.${state}`);
  });

  // GeneratingPanel + ResultsPanel's H2 summary both key off PIPELINE_STEP_KEYS.
  const stepKeyCases = LOCALES.flatMap((lng) =>
    PIPELINE_STEP_KEYS.map((key) => [lng, key] as const)
  );
  it.each(stepKeyCases)('%s pipeline.step.%s.{label,description}', (lng, key) => {
    expectRealCopy(lng, `pipeline.step.${key}.label`);
    expectRealCopy(lng, `pipeline.step.${key}.description`);
  });

  // Terminal-state announcer (GeneratingPanel).
  it.each(LOCALES)('%s pipeline.step.allDone', (lng) => {
    expectRealCopy(lng, 'pipeline.step.allDone');
  });

  // M3: streaming-target header — a closed 2-way ternary, not a template
  // literal, but the same "reachable dynamic key" shape.
  const targetCases = LOCALES.flatMap((lng) =>
    (['resume', 'cover'] as const).map((target) => [lng, target] as const)
  );
  it.each(targetCases)('%s autopilot.apply.target.%s', (lng, target) => {
    expectRealCopy(lng, `autopilot.apply.target.${target}`);
  });

  // CR-3: the literal (non-dynamic) keys this PR added or renamed to
  // ResultsPanel — a typo here between the call site and the catalog is the
  // same class of "renders a dotted key" failure a mocked `t` cannot catch.
  const literalKeys = [
    'autopilot.apply.wizard.results.needsReviewTitleEmpty',
    'autopilot.apply.wizard.results.needsReviewHintEmpty',
    'autopilot.apply.wizard.results.needsReviewHintCold',
    'autopilot.apply.wizard.results.failedTitle',
    'autopilot.apply.wizard.results.failedHint',
    'autopilot.apply.wizard.results.cancelledHint',
    'pipeline.runs.title',
  ] as const;
  const literalCases = LOCALES.flatMap((lng) => literalKeys.map((path) => [lng, path] as const));
  it.each(literalCases)('%s resolves %s', (lng, path) => {
    expectRealCopy(lng, path);
  });
});
