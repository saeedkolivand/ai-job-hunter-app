import { describe, expect, it } from 'vitest';

import { GOLD, renderLineChart } from './line-chart-svg.mjs';

/**
 * A series long enough to reach every branch: the peak annotation is skipped
 * below four points, and the steep step at index 5 gives peakStep() something
 * to label.
 */
function series(n = 12) {
  return Array.from({ length: n }, (_, i) => ({
    date: `2026-08-${String(i + 1).padStart(2, '0')}`,
    value: i < 5 ? i * 2 : i * 2 + 40,
  }));
}

const chart = (over = {}) =>
  renderLineChart({
    points: series(),
    title: 'GitHub stars',
    accent: GOLD,
    subtitle: 'since the repo went public',
    noun: 'stars',
    ...over,
  });

describe('renderLineChart', () => {
  // These four marks are not an arbitrary markup checklist — they are exactly
  // what the animated build failed to paint. Embedded via <img> (README through
  // camo, and /  through public/), Chrome applied the draw-on animation but
  // never advanced its timeline, so animation-fill-mode:backwards pinned the
  // from-state and the card, title, gridlines and axis labels rendered while
  // the data itself did not. A chart that passes "is it an SVG" while showing
  // no data is the failure this file's header exists to prevent.
  it('paints the fill, both pencil passes, the endpoint and the annotation', () => {
    const svg = chart();

    expect(svg).toContain('<polygon points="'); // area fill under the curve
    expect(svg.match(/<polyline points="/g) ?? []).toHaveLength(2); // retraced line
    expect(svg).toMatch(/<circle cx="[\d.]+" cy="[\d.]+" r="5"/); // latest-value dot
    expect(svg).toMatch(/\+\d+ stars</); // handwritten peak note
  });

  // The invariant this whole file is built around. Any animation here is
  // invisible-by-default at both embed sites, so it must fail loudly in CI
  // rather than ship a chart with no line in it again.
  it('declares nothing that animates', () => {
    const svg = chart();

    expect(svg).not.toMatch(/@keyframes/i);
    expect(svg).not.toMatch(/\banimation(-[a-z]+)?\s*:/i);
    expect(svg).not.toMatch(/<(animate|animateTransform|animateMotion|set)\b/i);
    expect(svg).not.toMatch(/class="[^"]*\b(draw|fade)\b/i);
  });

  // The <img> alt on the home page cannot carry these numbers (the SVG is
  // gitignored and absent at build time), so the file's own label is the only
  // place the current figure is announced.
  it('labels itself with the latest value and date', () => {
    const svg = chart();
    const latest = series().at(-1);

    expect(svg).toContain('role="img"');
    expect(svg).toContain(`aria-label="GitHub stars: ${latest.value} as of ${latest.date}"`);
    expect(svg).toContain('<title>');
    expect(svg).toContain('<desc>');
  });

  it('refuses an empty series rather than emitting an empty chart', () => {
    expect(() => chart({ points: [] })).toThrow(/no points/);
  });
});
