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

  // Posters and clips are hosted differently on purpose: the stills are ~2 MB
  // total and ship in `public/` so the route paints before anything is fetched,
  // while the 62 MB of clips come from the R2 bucket behind cdn.aijobhunter.app.
  //
  // Asserted against LITERALS, not against the config's own base, so a
  // regression that breaks the URL cannot move the expectation along with it.
  // The host matters as much as the shape: `scrub-engine` loads clips with
  // `fetch().then(r => r.blob())`, so they must come from an origin that sends
  // `Access-Control-Allow-Origin`. A same-origin `/world/vid/...` path or a
  // GitHub release URL both LOOK fine and both break the route.
  const CLIP_URL = /^https:\/\/cdn\.aijobhunter\.app\/world\/vid\/[\w.-]+\.mp4$/;

  it('serves every poster still from /world/ in the deployed site', () => {
    for (const section of sections) {
      expect(section.still.startsWith('/world/')).toBe(true);
      expect(section.stillMobile.startsWith('/world/')).toBe(true);
    }
  });

  it('serves every clip from the CORS-enabled asset host, never from the site itself', () => {
    for (const section of sections) {
      expect(section.clip).toMatch(CLIP_URL);
      expect(section.clipMobile).toMatch(CLIP_URL);
    }
  });

  it('serves every connector from that same host', () => {
    for (const path of [...connectors, ...connectorsMobile]) {
      expect(path).toMatch(CLIP_URL);
    }
  });

  it('serves every clip from ONE origin, so a half-finished migration cannot ship', () => {
    const origins = new Set(
      [...sections.flatMap((s) => [s.clip, s.clipMobile]), ...connectors, ...connectorsMobile].map(
        (url) => new URL(url).origin
      )
    );
    expect(origins.size).toBe(1);
    expect([...origins][0]).toBe('https://cdn.aijobhunter.app');
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
