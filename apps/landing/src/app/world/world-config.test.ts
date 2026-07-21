import { describe, expect, it } from 'vitest';

import { withAv1Sources, WORLD_CONFIG } from './world-config';

// Typed-data invariants for the /world scroll-flight config. The media assets
// themselves are rendered separately and land in public/world/ later — this
// only checks the config's own shape and that every path is wired to the
// site-root /world/ convention, never that the files exist on disk yet.
describe('world-config data integrity', () => {
  const { sections, connectors, connectorsMobile } = WORLD_CONFIG;

  it('has one fewer connector than sections (a connector links each adjacent pair)', () => {
    expect(connectors.length).toBe(sections.length - 1);
    expect(connectorsMobile.length).toBe(sections.length - 1);
  });

  it('gives every section required UI strings and media paths', () => {
    for (const section of sections) {
      expect(section.id).toBeTruthy();
      expect(section.still).toBeTruthy();
      expect(section.clip).toBeTruthy();
      expect(section.title).toBeTruthy();
      expect(section.label).toBeTruthy();
      expect(section.accent).toBeTruthy();
      expect(section.eyebrow).toBeTruthy();
      expect(section.body).toBeTruthy();
    }
  });

  it('gives every section a unique id', () => {
    const ids = new Set(sections.map((s) => s.id));
    expect(ids.size).toBe(sections.length);
  });

  it('only the last section carries a CTA', () => {
    for (const section of sections.slice(0, -1)) expect(section.cta).toBeUndefined();
    expect(sections.at(-1)?.cta).toBeDefined();
  });

  it('roots every section asset path under /world/ (media lands later, not checked on disk)', () => {
    for (const section of sections) {
      expect(section.still.startsWith('/world/')).toBe(true);
      expect(section.stillMobile.startsWith('/world/')).toBe(true);
      expect(section.clip.startsWith('/world/')).toBe(true);
      expect(section.clipMobile.startsWith('/world/')).toBe(true);
    }
  });

  it('roots every connector asset path under /world/', () => {
    for (const path of [...connectors, ...connectorsMobile]) {
      expect(path.startsWith('/world/')).toBe(true);
    }
  });
});

describe('withAv1Sources', () => {
  it('rewrites every section clip and connector to the .av1.mp4 encode', () => {
    const av1Config = withAv1Sources(WORLD_CONFIG);
    expect(av1Config.sections.map((s) => s.clip)).toEqual(
      WORLD_CONFIG.sections.map((s) => s.clip.replace(/\.mp4$/, '.av1.mp4'))
    );
    expect(av1Config.connectors).toEqual(
      WORLD_CONFIG.connectors.map((c) => c.replace(/\.mp4$/, '.av1.mp4'))
    );
  });

  it('leaves mobile clips, mobile connectors, and every still/poster untouched', () => {
    const av1Config = withAv1Sources(WORLD_CONFIG);
    expect(av1Config.sections.map((s) => s.clipMobile)).toEqual(
      WORLD_CONFIG.sections.map((s) => s.clipMobile)
    );
    expect(av1Config.sections.map((s) => s.still)).toEqual(
      WORLD_CONFIG.sections.map((s) => s.still)
    );
    expect(av1Config.sections.map((s) => s.stillMobile)).toEqual(
      WORLD_CONFIG.sections.map((s) => s.stillMobile)
    );
    expect(av1Config.connectorsMobile).toEqual(WORLD_CONFIG.connectorsMobile);
  });

  it('does not mutate the input config', () => {
    const snapshot = JSON.parse(JSON.stringify(WORLD_CONFIG));
    withAv1Sources(WORLD_CONFIG);
    expect(WORLD_CONFIG).toEqual(snapshot);
  });
});
