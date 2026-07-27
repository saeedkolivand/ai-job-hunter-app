// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mountScrollWorld } from './scrub-engine';

// Behavioural regression tests for the two device bugs the vendored engine shipped
// with: iPhones showing every scene past the first as a frozen poster, and iPads
// being served the low-quality portrait phone encodes. The engine is vanilla JS
// with no exported internals, so everything here is asserted through what it does
// to the DOM/network — which is also the only surface a re-vendor could break.
//
// jsdom never decodes media, so `loadedmetadata` / `loadeddata` / `seeked` never
// fire here. That is exactly the iOS failure mode these tests need to reproduce:
// a clip whose events never arrive must still be primed and must still recover.

const CLIP_TIMEOUT = 0;

const flush = () => new Promise<void>((resolve) => setTimeout(() => resolve(), CLIP_TIMEOUT));

/** The three media queries the engine probes, driven per-test. */
function stubMatchMedia(opts: { coarse: boolean; narrow: boolean; reduce: boolean }) {
  vi.stubGlobal('matchMedia', (query: string) => {
    let matches = opts.narrow; // '(max-width: 860px)'
    if (query.includes('prefers-reduced-motion')) matches = opts.reduce;
    else if (query.includes('pointer: coarse')) matches = opts.coarse;
    return { matches, media: query, addEventListener() {}, removeEventListener() {} };
  });
}

function stubScreen(width: number, height: number) {
  Object.defineProperty(window.screen, 'width', { configurable: true, value: width });
  Object.defineProperty(window.screen, 'height', { configurable: true, value: height });
}

/** Unique asset paths per test so earlier mounts' listeners can't pollute a filter. */
function configFor(prefix: string) {
  const section = (n: number) => ({
    id: `${prefix}-${n}`,
    label: `S${n}`,
    title: `S${n}`,
    accent: 'rebeccapurple',
    still: `/${prefix}/d${n}.png`,
    stillMobile: `/${prefix}/m${n}.png`,
    clip: `/${prefix}/d${n}.mp4`,
    clipMobile: `/${prefix}/m${n}.mp4`,
  });
  return {
    nav: false,
    atmosphere: false,
    sections: [section(0), section(1)],
    connectors: [`/${prefix}/c0.mp4`],
    connectorsMobile: [`/${prefix}/c0m.mp4`],
  };
}

function mount(prefix: string) {
  const container = document.createElement('div');
  document.body.appendChild(container);
  mountScrollWorld(container, configFor(prefix));
  return container;
}

/** Records every fetched URL; the returned promise factory controls resolution. */
function stubFetch(mode: 'pending' | 'ok') {
  const urls: string[] = [];
  vi.stubGlobal('fetch', (url: string) => {
    urls.push(url);
    if (mode === 'pending') return new Promise<Response>(() => {});
    const response = { ok: true, blob: () => Promise.resolve(new Blob()) };
    return Promise.resolve(response as unknown as Response);
  });
  return urls;
}

/** Captures which <video> elements the engine tried to prime (muted play→pause). */
function trackPriming(playResult: 'resolve' | 'reject') {
  const played: HTMLMediaElement[] = [];
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function (
    this: HTMLMediaElement
  ) {
    played.push(this);
    return playResult === 'resolve'
      ? Promise.resolve()
      : Promise.reject(new Error('gesture required'));
  });
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
  return played;
}

const primedIn = (played: HTMLMediaElement[], container: HTMLElement) =>
  played.filter((v) => container.contains(v));

const originalCreateObjectURL = URL.createObjectURL;

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  URL.createObjectURL = originalCreateObjectURL;
  document.body.innerHTML = '';
});

// ---------------------------------------------------------------------------
// Bug B — a coarse pointer alone used to mean "phone", so every tablet and touch
// laptop got the 720x1280 crf28 portrait encodes (and desktop CSS then cropped
// them). Mobile now means coarse AND a phone-sized screen, measured on the
// screen's SHORT side so rotating the device can't flip the decision mid-session.
// ---------------------------------------------------------------------------
describe('device classification picks the asset set', () => {
  const CASES = [
    { name: 'iPhone 15 Pro Max', coarse: true, narrow: true, w: 430, h: 932, set: 'mobile' },
    { name: 'Android phone', coarse: true, narrow: true, w: 412, h: 915, set: 'mobile' },
    { name: 'iPad mini portrait', coarse: true, narrow: true, w: 744, h: 1133, set: 'desktop' },
    { name: 'iPad mini landscape', coarse: true, narrow: false, w: 1133, h: 744, set: 'desktop' },
    { name: 'Android tablet', coarse: true, narrow: true, w: 800, h: 1280, set: 'desktop' },
    { name: 'touch laptop', coarse: true, narrow: false, w: 1512, h: 982, set: 'desktop' },
    { name: 'desktop', coarse: false, narrow: false, w: 2560, h: 1440, set: 'desktop' },
    // Preserved legacy behaviour: a non-touch window narrower than 860px still
    // gets the light encodes — now frozen at mount instead of re-read per clip.
    { name: 'narrow desktop window', coarse: false, narrow: true, w: 2560, h: 1440, set: 'mobile' },
  ] as const;

  it.each(CASES)('$name gets the $set set', ({ coarse, narrow, w, h, set }) => {
    vi.stubGlobal('requestAnimationFrame', () => 0);
    stubMatchMedia({ coarse, narrow, reduce: false });
    stubScreen(w, h);
    const urls = stubFetch('pending');
    const container = mount('cls');

    const poster = container.querySelector('.sw-scene__still')?.getAttribute('src');
    expect(poster).toBe(set === 'mobile' ? '/cls/m0.png' : '/cls/d0.png');
    expect(urls).toContain(set === 'mobile' ? '/cls/m0.mp4' : '/cls/d0.mp4');
    expect(urls).not.toContain(set === 'mobile' ? '/cls/d0.mp4' : '/cls/m0.mp4');
  });
});

// ---------------------------------------------------------------------------
// Bug A — the primer was registered {once:true} and only reached the clips that
// already existed at the first touch (segment 0). iOS refuses to load media data
// for a video created outside a gesture, so every later clip stayed event-less
// and its scene never left the poster.
// ---------------------------------------------------------------------------
describe('iOS video priming', () => {
  function phoneWithVideos(playResult: 'resolve' | 'reject') {
    vi.stubGlobal('requestAnimationFrame', () => 0);
    stubMatchMedia({ coarse: true, narrow: true, reduce: false });
    stubScreen(430, 932);
    URL.createObjectURL = () => 'blob:scrub-engine-test';
    return { played: trackPriming(playResult) };
  }

  it('primes clips that appear after the touch (their load events never fire on iOS)', async () => {
    const { played } = phoneWithVideos('resolve');
    const resolvers: Array<() => void> = [];
    vi.stubGlobal(
      'fetch',
      () =>
        new Promise<Response>((resolve) => {
          const response = { ok: true, blob: () => Promise.resolve(new Blob()) };
          resolvers.push(() => resolve(response as unknown as Response));
        })
    );
    const container = mount('late');

    // The touch lands while the clips are still in flight — nothing to prime yet.
    window.dispatchEvent(new Event('pointerdown'));
    expect(primedIn(played, container)).toHaveLength(0);

    resolvers.forEach((resolve) => resolve());
    await flush();

    const videos = container.querySelectorAll('video.sw-scene__video');
    expect(videos.length).toBeGreaterThan(0);
    expect(primedIn(played, container)).toHaveLength(videos.length);
  });

  it('re-primes on a later touch when play() was refused (the listener is not once-only)', async () => {
    const { played } = phoneWithVideos('reject');
    stubFetch('ok');
    const container = mount('retry');
    await flush();

    // Clips created before any gesture must not be primed yet.
    expect(primedIn(played, container)).toHaveLength(0);

    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    const afterFirstTouch = primedIn(played, container).length;
    expect(afterFirstTouch).toBeGreaterThan(0);

    // A once:true listener would be gone by now and this would stay flat.
    window.dispatchEvent(new Event('touchstart'));
    await flush();
    expect(primedIn(played, container)).toHaveLength(afterFirstTouch * 2);
  });

  it('primes each clip only once while play() keeps succeeding', async () => {
    const { played } = phoneWithVideos('resolve');
    stubFetch('ok');
    const container = mount('once');
    await flush();

    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    const primed = primedIn(played, container).length;

    window.dispatchEvent(new Event('pointerdown'));
    window.dispatchEvent(new Event('touchstart'));
    await flush();
    expect(primedIn(played, container)).toHaveLength(primed);
  });
});

// ---------------------------------------------------------------------------
// Bug A hardening — a successful fetch latched `loading` forever, so a clip that
// then failed to decode wedged its scene; a failing fetch did the opposite and
// re-requested on every scroll tick.
// ---------------------------------------------------------------------------
describe('clip failure recovery', () => {
  it('drops the dead video back to its poster and stops retrying after 3 attempts', async () => {
    vi.stubGlobal('requestAnimationFrame', () => 0);
    stubMatchMedia({ coarse: true, narrow: true, reduce: false });
    stubScreen(430, 932);
    URL.createObjectURL = () => 'blob:scrub-engine-test';
    trackPriming('resolve');
    const urls = stubFetch('ok');
    const container = mount('fail');
    await flush();

    const scene = container.querySelector('.sw-scene');
    if (!scene) throw new Error('engine did not build a scene');
    // The engine only adds has-clip on `seeked`, which jsdom never fires; set it
    // by hand so the error path's cleanup of it is actually observable.
    scene.classList.add('has-clip');
    const failClip = () => scene.querySelector('video')?.dispatchEvent(new Event('error'));

    failClip();
    expect(scene.querySelector('video')).toBeNull();
    expect(scene.classList.contains('has-clip')).toBe(false);

    const attempts = () => urls.filter((u) => u === '/fail/m0.mp4').length;
    expect(attempts()).toBe(1);

    // orientationchange re-runs layout()→read() synchronously, i.e. a scroll tick.
    for (let i = 0; i < 5; i++) {
      window.dispatchEvent(new Event('orientationchange'));
      await flush();
      failClip();
    }
    expect(attempts()).toBe(3);
  });
});
