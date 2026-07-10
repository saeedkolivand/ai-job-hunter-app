import { describe, expect, it, vi } from 'vitest';

import { TEMPLATE_IDS } from '@/lib/generate';

// cover-template-previews.ts runs import.meta.glob at module scope.
// Vitest does not implement Vite's glob transform in jsdom, so we mock the
// module and supply a stub that mirrors what Vite would produce: one entry per
// SVG file, keyed by a path whose basename (minus extension) is the template id.
vi.mock('./cover-template-previews', () => {
  // Simulate the 9 committed SVG assets.
  const ids = [
    'classic',
    'swiss-minimal',
    'academic',
    'atelier',
    'meridian',
    'throughline',
    'portrait',
    'lebenslauf',
  ] as const;

  const COVER_TEMPLATE_PREVIEWS = Object.fromEntries(
    ids.map((id) => [id, `/assets/cover-template-previews/${id}.svg`])
  );

  return { COVER_TEMPLATE_PREVIEWS };
});

describe('COVER_TEMPLATE_PREVIEWS', () => {
  it('exports a URL for every canonical TemplateId', async () => {
    const { COVER_TEMPLATE_PREVIEWS } = await import('./cover-template-previews');

    for (const id of TEMPLATE_IDS) {
      expect(
        COVER_TEMPLATE_PREVIEWS[id],
        `Missing cover preview for template "${id}"`
      ).toBeTruthy();
    }
  });

  it('contains no extra or unknown keys', async () => {
    const { COVER_TEMPLATE_PREVIEWS } = await import('./cover-template-previews');
    const knownIds = new Set<string>(TEMPLATE_IDS);

    for (const key of Object.keys(COVER_TEMPLATE_PREVIEWS)) {
      expect(knownIds.has(key), `Unexpected key "${key}" in COVER_TEMPLATE_PREVIEWS`).toBe(true);
    }
  });

  it('every URL value is a non-empty string', async () => {
    const { COVER_TEMPLATE_PREVIEWS } = await import('./cover-template-previews');

    for (const [id, url] of Object.entries(COVER_TEMPLATE_PREVIEWS)) {
      expect(typeof url, `URL for "${id}" is not a string`).toBe('string');
      expect(url.length, `URL for "${id}" is empty`).toBeGreaterThan(0);
    }
  });
});
