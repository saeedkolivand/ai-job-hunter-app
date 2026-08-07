/**
 * MatchBand — i18n key-drift guard for the hover description.
 *
 * A separate file from `match-band.test.tsx` on purpose: that one stubs
 * `@ajh/translations` with `t: (k) => k`, which echoes raw keys back and so can
 * never catch a mistyped or missing resource entry. The description key is
 * built at runtime from tier + variant (`jobs.matchBand.desc.<variant>.<tier>`),
 * so TypeScript cannot check it either — only a real `t()` can.
 *
 * Same rationale and setup as `trust-badge.test.tsx`: `@ajh/translations`
 * resolves to real source under vitest and initializes with the real bundled
 * en/de resources, so a broken key surfaces as the raw key string in the DOM.
 */

import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { MatchBand, matchBandDescriptionKey, scoreTier } from './match-band';

const VARIANTS = ['combined', 'coverage'] as const;
const SAMPLES = [
  { value: 90, tier: 'High' },
  { value: 52, tier: 'Medium' },
  { value: 5, tier: 'Low' },
] as const;

describe('MatchBand description copy resolves for every tier and variant', () => {
  for (const variant of VARIANTS) {
    for (const { value, tier } of SAMPLES) {
      it(`${variant}/${tier} resolves to real copy, not a raw key`, () => {
        const key = matchBandDescriptionKey(scoreTier(value, variant).key, variant);
        const { container } = render(<MatchBand value={value} variant={variant} />);
        const title = container.querySelector('[title]')?.getAttribute('title') ?? '';

        expect(title).not.toBe(key);
        expect(title).not.toContain('jobs.matchBand');
        // A description that doesn't say anything is the same failure as a
        // missing one — a bare word would leave the badge unexplained.
        expect(title.length).toBeGreaterThan(20);
      });
    }
  }

  it('says something different for coverage than for combined', () => {
    // The two make genuinely different claims: coverage is keyword overlap with
    // no AI scoring, combined blends meaning and keywords. Identical copy here
    // would mean one of them is lying to the user.
    const read = (variant: (typeof VARIANTS)[number]) =>
      render(<MatchBand value={90} variant={variant} />)
        .container.querySelector('[title]')
        ?.getAttribute('title');

    expect(read('combined')).not.toBe(read('coverage'));
  });

  it('distinguishes the tiers from each other', () => {
    const titles = SAMPLES.map(({ value }) =>
      render(<MatchBand value={value} />)
        .container.querySelector('[title]')
        ?.getAttribute('title')
    );
    expect(new Set(titles).size).toBe(SAMPLES.length);
  });
});
