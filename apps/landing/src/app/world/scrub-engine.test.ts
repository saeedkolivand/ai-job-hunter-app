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

const flush = () => new Promise<void>((resolve) => setTimeout(() => resolve(), 0));

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

/** Drives the priming gate — deliberately independent of the coarse-pointer query. */
function stubTouchPoints(points: number) {
  Object.defineProperty(window.navigator, 'maxTouchPoints', { configurable: true, value: points });
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
function trackPriming(playResult: 'resolve' | 'reject' | 'no-promise') {
  const played: HTMLMediaElement[] = [];
  const paused: HTMLMediaElement[] = [];
  vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(function (
    this: HTMLMediaElement
  ) {
    played.push(this);
    if (playResult === 'no-promise') return undefined as unknown as Promise<void>;
    return playResult === 'resolve'
      ? Promise.resolve()
      : Promise.reject(new Error('gesture required'));
  });
  vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function (
    this: HTMLMediaElement
  ) {
    paused.push(this);
  });
  // The engine calls load() when tearing a failed clip down; jsdom's stub only emits
  // a virtual-console error, so silence it to keep failures readable.
  vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => {});
  return { played, paused };
}

/**
 * Makes a <video> scrubbable in jsdom (which has no media stack) and reports the
 * currentTime the engine seeks it to.
 */
function fakeMedia(video: HTMLVideoElement) {
  const state = { currentTime: 0 };
  Object.defineProperty(video, 'duration', { configurable: true, value: 1 });
  Object.defineProperty(video, 'seeking', { configurable: true, value: false });
  Object.defineProperty(video, 'currentTime', {
    configurable: true,
    get: () => state.currentTime,
    set: (t: number) => {
      state.currentTime = t;
    },
  });
  return state;
}

/** Captures the engine's rAF callbacks so a single frame can be driven by hand. */
function captureFrames() {
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => frames.push(cb));
  return () => {
    const frame = frames[0];
    if (!frame) throw new Error('engine never scheduled a frame');
    frame(0);
  };
}

/** Puts the page mid-scroll so segment 0 has a non-zero seek target. */
function stubScrollY(y: number) {
  Object.defineProperty(window, 'scrollY', { configurable: true, value: y });
}

const inside = (els: HTMLMediaElement[], container: HTMLElement) =>
  els.filter((v) => container.contains(v));

const originalCreateObjectURL = URL.createObjectURL;
const originalRevokeObjectURL = URL.revokeObjectURL;

/** A touch device whose clips actually materialise as <video> elements. */
function touchDeviceWithVideos(playResult: 'resolve' | 'reject' | 'no-promise') {
  vi.stubGlobal('requestAnimationFrame', () => 0);
  stubMatchMedia({ coarse: true, narrow: true, reduce: false });
  stubScreen(430, 932);
  stubTouchPoints(5);
  URL.createObjectURL = () => 'blob:scrub-engine-test';
  URL.revokeObjectURL = () => {};
  return trackPriming(playResult);
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  URL.createObjectURL = originalCreateObjectURL;
  URL.revokeObjectURL = originalRevokeObjectURL;
  stubTouchPoints(0);
  stubScrollY(0);
  document.body.replaceChildren();
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

  // The third consumer of the frozen classification, and the one the poster/clip
  // assertions above can't see: raf()'s seek step (`eps`). A re-vendor that restored
  // a live `isMobile()` at this call site alone would slip past every other test.
  it('scrubs a tablet at the desktop seek step and a phone at the coarser one', async () => {
    async function seekAfterOneFrame(prefix: string, w: number, h: number) {
      const runFrame = captureFrames();
      stubMatchMedia({ coarse: true, narrow: true, reduce: false });
      stubScreen(w, h);
      stubTouchPoints(5);
      URL.createObjectURL = () => 'blob:scrub-engine-test';
      trackPriming('resolve');
      stubFetch('ok');
      const container = mount(prefix);
      await flush();

      const video = container.querySelector('video');
      if (!video) throw new Error('engine did not create a clip');
      const media = fakeMedia(video);
      video.dispatchEvent(new Event('loadedmetadata'));
      runFrame();
      return media.currentTime;
    }

    // Same scroll offset and the same 0.18 lerp for both, so the single lerped step
    // is identical — it just lands between the two thresholds (0.008 and 0.02).
    stubScrollY(60);
    const tablet = await seekAfterOneFrame('epstab', 800, 1280);
    const phone = await seekAfterOneFrame('epsphone', 430, 932);

    expect(tablet).toBeGreaterThan(0.008);
    expect(tablet).toBeLessThan(0.02);
    expect(phone).toBe(0); // below the coarse step, so the seek is coalesced away
  });
});

// ---------------------------------------------------------------------------
// Bug A — the primer was registered {once:true} and only reached the clips that
// already existed at the first touch (segment 0). iOS refuses to load media data
// for a video created outside a gesture, so every later clip stayed event-less
// and its scene never left the poster.
// ---------------------------------------------------------------------------
describe('iOS video priming', () => {
  it('primes clips that appear after the touch (their load events never fire on iOS)', async () => {
    const { played } = touchDeviceWithVideos('resolve');
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
    expect(inside(played, container)).toHaveLength(0);

    resolvers.forEach((resolve) => resolve());
    await flush();

    const videos = container.querySelectorAll('video.sw-scene__video');
    expect(videos.length).toBeGreaterThan(0);
    expect(inside(played, container)).toHaveLength(videos.length);
  });

  it('re-primes on a later touch when play() was refused (the listener is not once-only)', async () => {
    const { played } = touchDeviceWithVideos('reject');
    stubFetch('ok');
    const container = mount('retry');
    await flush();

    // Clips created before any gesture must not be primed yet.
    expect(inside(played, container)).toHaveLength(0);

    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    const afterFirstTouch = inside(played, container).length;
    expect(afterFirstTouch).toBeGreaterThan(0);

    // A once:true listener would be gone by now and this would stay flat.
    window.dispatchEvent(new Event('touchstart'));
    await flush();
    expect(inside(played, container)).toHaveLength(afterFirstTouch * 2);
  });

  it('primes each clip only once while play() keeps succeeding', async () => {
    const { played } = touchDeviceWithVideos('resolve');
    stubFetch('ok');
    const container = mount('once');
    await flush();

    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    const primed = inside(played, container).length;

    window.dispatchEvent(new Event('pointerdown'));
    window.dispatchEvent(new Event('touchstart'));
    await flush();
    expect(inside(played, container)).toHaveLength(primed);
  });

  it('primes a trackpad-attached iPad, which reports a fine pointer but is still touch', async () => {
    vi.stubGlobal('requestAnimationFrame', () => 0);
    // iPadOS 13.4+ with a Magic Keyboard: hover:hover + pointer:fine, yet WebKit
    // still gates media loading on a gesture. `coarse` misses this device entirely.
    stubMatchMedia({ coarse: false, narrow: false, reduce: false });
    stubScreen(1024, 1366);
    stubTouchPoints(5);
    URL.createObjectURL = () => 'blob:scrub-engine-test';
    const { played } = trackPriming('resolve');
    stubFetch('ok');
    const container = mount('ipadtrackpad');
    await flush();

    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    const videos = container.querySelectorAll('video.sw-scene__video');
    expect(videos.length).toBeGreaterThan(0);
    expect(inside(played, container)).toHaveLength(videos.length);
  });

  it('never primes on a non-touch desktop', async () => {
    vi.stubGlobal('requestAnimationFrame', () => 0);
    stubMatchMedia({ coarse: false, narrow: false, reduce: false });
    stubScreen(2560, 1440);
    stubTouchPoints(0);
    URL.createObjectURL = () => 'blob:scrub-engine-test';
    const { played } = trackPriming('resolve');
    stubFetch('ok');
    const container = mount('nontouch');
    await flush();

    expect(container.querySelectorAll('video.sw-scene__video').length).toBeGreaterThan(0);
    window.dispatchEvent(new Event('pointerdown'));
    window.dispatchEvent(new Event('touchstart'));
    await flush();
    expect(inside(played, container)).toHaveLength(0);
  });

  it('gives up re-priming after 3 refusals rather than retrying on every touch', async () => {
    const { played } = touchDeviceWithVideos('reject');
    stubFetch('ok');
    const container = mount('primecap');
    await flush();

    for (let i = 0; i < 6; i++) {
      window.dispatchEvent(new Event('pointerdown'));
      await flush();
    }

    const clips = container.querySelectorAll('video.sw-scene__video').length;
    expect(clips).toBeGreaterThan(0);
    expect(inside(played, container)).toHaveLength(clips * 3);
  });

  it('pauses straight away when play() returns no promise (pre-promise WebKit)', async () => {
    const { played, paused } = touchDeviceWithVideos('no-promise');
    stubFetch('ok');
    const container = mount('nopromise');
    await flush();

    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    expect(inside(played, container).length).toBeGreaterThan(0);
    expect(inside(paused, container)).toHaveLength(inside(played, container).length);
  });
});

// ---------------------------------------------------------------------------
// Bug A hardening — a successful fetch latched `loading` forever, so a clip that
// then failed to decode wedged its scene; a failing fetch did the opposite and
// re-requested on every scroll tick.
// ---------------------------------------------------------------------------
describe('clip failure recovery', () => {
  function sceneOf(container: HTMLElement) {
    const scene = container.querySelector('.sw-scene');
    if (!scene) throw new Error('engine did not build a scene');
    return scene;
  }

  it('drops the dead video back to its poster and stops retrying after 3 attempts', async () => {
    touchDeviceWithVideos('resolve');
    const revoked: string[] = [];
    URL.revokeObjectURL = (url: string) => void revoked.push(url);
    const urls = stubFetch('ok');
    const container = mount('fail');
    await flush();

    const scene = sceneOf(container);
    // The engine only adds has-clip on `seeked`, which jsdom never fires; set it
    // by hand so the error path's cleanup of it is actually observable.
    scene.classList.add('has-clip');
    const failClip = () => scene.querySelector('video')?.dispatchEvent(new Event('error'));

    failClip();
    expect(scene.querySelector('video')).toBeNull();
    expect(scene.classList.contains('has-clip')).toBe(false);
    // The discarded blob is released; live clips deliberately keep theirs.
    expect(revoked).toEqual(['blob:scrub-engine-test']);

    const attempts = () => urls.filter((u) => u === '/fail/m0.mp4').length;
    expect(attempts()).toBe(1);

    // orientationchange re-runs layout()→read() synchronously, i.e. a scroll tick.
    // NOTE: the engine has no teardown API, so every mount from an earlier test in
    // this file is still listening and will re-run its own read() on this event.
    // That is why `configFor` gives each mount unique asset paths and every
    // assertion here filters by prefix or by `container.contains`. A new assertion
    // over an unscoped global (total fetch count, prototype spy call counts, a
    // document-wide querySelectorAll) will pick up those foreign mounts — scope it.
    for (let i = 0; i < 5; i++) {
      window.dispatchEvent(new Event('orientationchange'));
      await flush();
      failClip();
    }
    expect(attempts()).toBe(3);
  });

  it('ignores late loadedmetadata / seeked from a video the segment has replaced', async () => {
    // Tablet-classified so eps is the fine 0.008 — a wrongly-ready segment would
    // visibly seek the live clip, which is what makes this assertion meaningful.
    stubScrollY(60);
    stubMatchMedia({ coarse: true, narrow: true, reduce: false });
    stubScreen(800, 1280);
    stubTouchPoints(5);
    URL.createObjectURL = () => 'blob:scrub-engine-test';
    URL.revokeObjectURL = () => {};
    trackPriming('resolve');
    const runFrame = captureFrames();
    stubFetch('ok');
    const container = mount('latemeta');
    await flush();

    const scene = sceneOf(container);
    const first = scene.querySelector('video');
    first?.dispatchEvent(new Event('error'));
    window.dispatchEvent(new Event('orientationchange'));
    await flush();

    const second = scene.querySelector('video');
    if (!second) throw new Error('engine did not retry the clip');
    expect(second).not.toBe(first);
    const media = fakeMedia(second);

    // The discarded element finally reports metadata and a seek. Unguarded, this
    // flags the segment ready and reveals it while the LIVE clip has no metadata,
    // so raf() then scrubs it against the `duration || 1` fallback.
    first?.dispatchEvent(new Event('loadedmetadata'));
    first?.dispatchEvent(new Event('seeked'));
    expect(scene.classList.contains('has-clip')).toBe(false);
    runFrame();
    expect(media.currentTime).toBe(0);

    // Sanity: the live element's own metadata does mark the segment ready.
    second.dispatchEvent(new Event('loadedmetadata'));
    runFrame();
    expect(media.currentTime).toBeGreaterThan(0);
  });

  it('ignores a late error from a video the segment has already replaced', async () => {
    touchDeviceWithVideos('resolve');
    stubFetch('ok');
    const container = mount('stale');
    await flush();

    const scene = sceneOf(container);
    const first = scene.querySelector('video');
    first?.dispatchEvent(new Event('error'));
    window.dispatchEvent(new Event('orientationchange'));
    await flush();

    const second = scene.querySelector('video');
    expect(second).toBeTruthy();
    expect(second).not.toBe(first);

    // The discarded element errors again; without the identity guard this resets
    // the segment and the next tick stacks a third <video> over the live one.
    first?.dispatchEvent(new Event('error'));
    window.dispatchEvent(new Event('orientationchange'));
    await flush();

    expect(scene.querySelectorAll('video')).toHaveLength(1);
    expect(scene.querySelector('video')).toBe(second);
  });
});
