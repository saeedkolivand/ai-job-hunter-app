/**
 * N1 regression: a mocked `t` that echoes the key verbatim is structurally
 * blind to a MISSING key — `ResultsPanel.test.tsx`'s mock passed even while
 * `pipeline.status.done` rendered the raw dotted key in production, because
 * the mock has no notion of "this key doesn't exist in the real catalog".
 *
 * This file imports the REAL locale JSON directly (not through i18next, not
 * through a mock — `resolveJsonModule` makes this a plain object import).
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

import { STOPPED_SUFFIX, UNKNOWN_STOPPED_SUFFIX } from '@/lib/stopped-reason';

import de from '../../../../../../../../packages/translations/src/locales/de/translation.json';
import en from '../../../../../../../../packages/translations/src/locales/en/translation.json';
import { PIPELINE_STEP_KEYS } from './lib/pipeline-steps';
import { PIPELINE_STATUS_KEY } from './ResultsPanel';

type Catalog = Record<string, unknown>;

/** Read a dotted path off a parsed locale object. */
function at(obj: Catalog, path: string): unknown {
  return path
    .split('.')
    .reduce<unknown>(
      (node, seg) => (node && typeof node === 'object' ? (node as Catalog)[seg] : undefined),
      obj
    );
}

/**
 * CR-5: `toHaveProperty` passes for an empty string — a key that exists but
 * carries no copy would slip through silently (and render as blank chrome,
 * not a loud dotted-key failure). Assert a REAL, non-empty string in BOTH
 * locales, not just presence.
 */
function expectRealCopy(path: string) {
  for (const [label, catalog] of [
    ['en', en],
    ['de', de],
  ] as const) {
    const value = at(catalog, path);
    expect(typeof value, `${label}.${path} should be a string`).toBe('string');
    expect((value as string).length, `${label}.${path} should not be empty`).toBeGreaterThan(0);
  }
}

describe('ResultsPanel i18n — every reachable key resolves to real content', () => {
  // N1: `pipeline.status.*` — the Tag's key, driven by TailorRunState via
  // PIPELINE_STATUS_KEY. This is the exact bug: `pipeline.status.done` and
  // `pipeline.status.error` do not exist — the vocabulary is
  // running/completed/needsReview/failed/cancelled.
  it.each(Object.entries(PIPELINE_STATUS_KEY))(
    'runState=%s -> pipeline.status.%s',
    (_runState, statusKey) => {
      expectRealCopy(`pipeline.status.${statusKey}`);
    }
  );

  // CR-3: `pipeline.stopped.${suffix}` (ResultsPanel's stopped-reason line) —
  // the reachable set is every `StoppedSuffix` VALUE `stoppedSuffix()` can
  // produce, plus its own unknown-reason fallback. Iterated from the shared
  // helper's own tables, not a hand-copied list, so a variant added there is
  // covered here automatically.
  it.each([...new Set(Object.values(STOPPED_SUFFIX)), UNKNOWN_STOPPED_SUFFIX])(
    'pipeline.stopped.%s',
    (suffix) => {
      expectRealCopy(`pipeline.stopped.${suffix}`);
    }
  );

  // H8: pipeline.step.state.{done,active,pending} — the per-row sr-only word.
  it.each(['done', 'active', 'pending'])('pipeline.step.state.%s', (state) => {
    expectRealCopy(`pipeline.step.state.${state}`);
  });

  // GeneratingPanel + ResultsPanel's H2 summary both key off PIPELINE_STEP_KEYS.
  it.each(PIPELINE_STEP_KEYS)('pipeline.step.%s.{label,description}', (key) => {
    expectRealCopy(`pipeline.step.${key}.label`);
    expectRealCopy(`pipeline.step.${key}.description`);
  });

  // Terminal-state announcer (GeneratingPanel).
  it('pipeline.step.allDone', () => {
    expectRealCopy('pipeline.step.allDone');
  });

  // M3: streaming-target header — a closed 2-way ternary, not a template
  // literal, but the same "reachable dynamic key" shape.
  it.each(['resume', 'cover'])('autopilot.apply.target.%s', (target) => {
    expectRealCopy(`autopilot.apply.target.${target}`);
  });

  // CR-3: the literal (non-dynamic) keys this PR added or renamed to
  // ResultsPanel — a typo here between the call site and the catalog is the
  // same class of "renders a dotted key" failure a mocked `t` cannot catch.
  it.each([
    'autopilot.apply.wizard.results.needsReviewTitleEmpty',
    'autopilot.apply.wizard.results.needsReviewHintEmpty',
    'autopilot.apply.wizard.results.needsReviewHintCold',
    'autopilot.apply.wizard.results.failedTitle',
    'autopilot.apply.wizard.results.failedHint',
    'autopilot.apply.wizard.results.cancelledHint',
    'pipeline.runs.title',
  ])('%s', (path) => {
    expectRealCopy(path);
  });
});
