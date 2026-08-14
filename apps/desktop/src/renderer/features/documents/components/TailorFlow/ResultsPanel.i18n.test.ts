/**
 * N1 regression: a mocked `t` that echoes the key verbatim is structurally
 * blind to a MISSING key — `ResultsPanel.test.tsx`'s mock passed even while
 * `pipeline.status.done` rendered the raw dotted key in production, because
 * the mock has no notion of "this key doesn't exist in the real catalog".
 *
 * This file imports the REAL locale JSON directly (not through i18next, not
 * through a mock — `resolveJsonModule` makes this a plain object import) and
 * asserts every key this component can actually reach at runtime resolves to
 * real content in BOTH locales — a sweep of every dynamically-constructed key
 * path this PR introduced or touched, so a future addition to a closed union
 * (a new `TailorRunState`, a new step key) fails loudly here instead of
 * rendering a dotted key in production.
 */
import { describe, expect, it } from 'vitest';

import de from '../../../../../../../../packages/translations/src/locales/de/translation.json';
import en from '../../../../../../../../packages/translations/src/locales/en/translation.json';
import { PIPELINE_STEP_KEYS } from './lib/pipeline-steps';
import { PIPELINE_STATUS_KEY } from './ResultsPanel';

describe('ResultsPanel i18n — every reachable key resolves to real content (N1 sweep)', () => {
  // N1: `pipeline.status.*` — the Tag's key, driven by TailorRunState via
  // PIPELINE_STATUS_KEY. This is the exact bug: `pipeline.status.done` and
  // `pipeline.status.error` do not exist — the vocabulary is
  // running/completed/needsReview/failed/cancelled.
  it.each(Object.entries(PIPELINE_STATUS_KEY))(
    'runState=%s -> pipeline.status.%s exists in en AND de',
    (_runState, statusKey) => {
      expect(en.pipeline.status).toHaveProperty(statusKey);
      expect(de.pipeline.status).toHaveProperty(statusKey);
    }
  );

  // H8: pipeline.step.state.{done,active,pending} — the per-row sr-only word.
  it.each(['done', 'active', 'pending'])('pipeline.step.state.%s exists in en AND de', (state) => {
    expect(en.pipeline.step.state).toHaveProperty(state);
    expect(de.pipeline.step.state).toHaveProperty(state);
  });

  // GeneratingPanel + ResultsPanel's H2 summary both key off PIPELINE_STEP_KEYS.
  it.each(PIPELINE_STEP_KEYS)('pipeline.step.%s.{label,description} exist in en AND de', (key) => {
    expect(en.pipeline.step).toHaveProperty([key, 'label']);
    expect(en.pipeline.step).toHaveProperty([key, 'description']);
    expect(de.pipeline.step).toHaveProperty([key, 'label']);
    expect(de.pipeline.step).toHaveProperty([key, 'description']);
  });

  // Terminal-state announcer (GeneratingPanel).
  it('pipeline.step.allDone exists in en AND de', () => {
    expect(en.pipeline.step).toHaveProperty('allDone');
    expect(de.pipeline.step).toHaveProperty('allDone');
  });

  // M3: streaming-target header — a closed 2-way ternary, not a template
  // literal, but the same "reachable dynamic key" shape.
  it.each(['resume', 'cover'])('autopilot.apply.target.%s exists in en AND de', (target) => {
    expect(en.autopilot.apply.target).toHaveProperty(target);
    expect(de.autopilot.apply.target).toHaveProperty(target);
  });
});
