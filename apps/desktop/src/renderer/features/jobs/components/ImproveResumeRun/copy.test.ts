/**
 * Phase-7 copy: every key the improve-resume surfaces render exists in BOTH
 * locales and actually resolves.
 *
 * Two assertions per key, because either alone lies:
 *
 *  - `getResource(lng, …)` reads the locale's OWN resource, with no fallback.
 *    `t()` under `fallbackLng: 'en'` happily answers a de lookup with English,
 *    so a missing German key would pass an execution-only check.
 *  - `getFixedT(lng)(…)` actually EXECUTES the lookup (interpolation included),
 *    which a presence-only check would not — a `{{max}}` that never got a value
 *    still renders its own braces.
 *
 * Runtime-built keys (`steps.${key}`, `status.${status}`, `state.${state}`) are
 * exactly the ones TypeScript cannot check, which is why they are enumerated
 * here rather than trusted.
 */
import { describe, expect, it } from 'vitest';

import i18n from '@ajh/translations';

const LOCALES = ['en', 'de'] as const;

const IMPROVE = 'jobs.tailored.improve';

/** Every key the entry action, the run card and the confirm editor render. */
const KEYS: string[] = [
  `${IMPROVE}.trigger`,
  `${IMPROVE}.triggerHint`,
  `${IMPROVE}.tooLong`,
  `${IMPROVE}.tooLongHint`,
  `${IMPROVE}.heading`,
  `${IMPROVE}.hint`,
  `${IMPROVE}.starting`,
  `${IMPROVE}.liveRegionLabel`,
  `${IMPROVE}.confirmAnnouncement`,
  `${IMPROVE}.summaryTitle`,
  `${IMPROVE}.doneHint`,
  `${IMPROVE}.cancelledHint`,
  `${IMPROVE}.failedTitle`,
  `${IMPROVE}.stop`,
  `${IMPROVE}.stopHint`,
  `${IMPROVE}.stopping`,
  `${IMPROVE}.dismiss`,
  ...(['running', 'waiting', 'done', 'cancelled', 'failed'] as const).map(
    (state) => `${IMPROVE}.state.${state}`
  ),
  ...(['pending', 'active', 'done', 'interrupted'] as const).map(
    (status) => `${IMPROVE}.status.${status}`
  ),
  ...(['report', 'check', 'trim', 'rewrite', 'evidence', 'save', 'summary'] as const).map(
    (step) => `${IMPROVE}.steps.${step}`
  ),
  // The confirm gate's résumé editor — without these, `save_resume` falls back
  // to read-only JSON and the flow's only output cannot be edited.
  'jobs.prep.confirm.tools.saveResume.summary',
  'jobs.prep.confirm.tools.saveResume.contentLabel',
];

describe.each(LOCALES)('improve-resume copy — %s', (lng) => {
  const t = i18n.getFixedT(lng);

  it.each(KEYS)('%s is present in this locale and resolves', (key) => {
    expect(typeof i18n.getResource(lng, 'translation', key)).toBe('string');
    const resolved = t(key, { max: 8000 });
    expect(resolved).not.toBe(key);
    expect(resolved.trim()).not.toBe('');
    // A leftover placeholder means the copy asks for a value the call sites
    // don't pass (or the other way round).
    expect(resolved).not.toMatch(/\{\{/);
  });

  it('interpolates the review cap into the refusal copy', () => {
    expect(t(`${IMPROVE}.tooLong`, { max: 8000 })).toContain('8000');
    expect(t(`${IMPROVE}.tooLongHint`, { max: 8000 })).toContain('8000');
  });
});
