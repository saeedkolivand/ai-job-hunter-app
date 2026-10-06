import { afterEach, vi } from 'vitest';

import { mountScrollWorld } from '../scrub-engine';

// Behavioural regression tests for the two device bugs the vendored engine shipped
// with: iPhones showing every scene past the first as a frozen poster, and iPads
// being served the low-quality portrait phone encodes. The engine is vanilla JS
// with no exported internals, so everything here is asserted through what it does
// to the DOM/network — which is also the only surface a re-vendor could break.
//
// jsdom never decodes media, so `loadedmetadata` / `loadeddata` / `seeked` never
// fire here. That is exactly the iOS failure mode these tests need to reproduce:
// a clip whose events never arrive must still be primed and must still recover.

export const flush = () => new Promise<void>((resolve) => setTimeout(() => resolve(), 0));

/** The three media queries the engine probes, driven per-test. */
export function stubMatchMedia(opts: { coarse: boolean; narrow: boolean; reduce: boolean }) {
  vi.stubGlobal('matchMedia', (query: string) => {
    let matches = opts.narrow; // '(max-width: 860px)'
    if (query.includes('prefers-reduced-motion')) matches = opts.reduce;
    else if (query.includes('pointer: coarse')) matches = opts.coarse;
    return { matches, media: query, addEventListener() {}, removeEventListener() {} };
  });
}

export function stubScreen(width: number, height: number) {
  Object.defineProperty(window.screen, 'width', { configurable: true, value: width });
  Object.defineProperty(window.screen, 'height', { configurable: true, value: height });
}

/** Drives the priming gate — deliberately independent of the coarse-pointer query. */
export function stubTouchPoints(points: number) {
  Object.defineProperty(window.navigator, 'maxTouchPoints', { configurable: true, value: points });
}

/** Unique asset paths per test so earlier mounts' listeners can't pollute a filter. */
export function configFor(prefix: string) {
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

/** Every mount's disposer, unwound in afterEach so mounts can't leak across tests. */
export const disposers: Array<() => void> = [];

export function mount(prefix: string) {
  const container = document.createElement('div');
  document.body.appendChild(container);
  disposers.push(mountScrollWorld(container, configFor(prefix)));
  return container;
}

/** Records every fetched URL; the returned promise factory controls resolution. */
export function stubFetch(mode: 'pending' | 'ok') {
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
export function trackPriming(playResult: 'resolve' | 'reject' | 'no-promise') {
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
export function fakeMedia(video: HTMLVideoElement) {
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
export function captureFrames() {
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => frames.push(cb));
  return () => {
    const frame = frames[0];
    if (!frame) throw new Error('engine never scheduled a frame');
    frame(0);
  };
}

/** Puts the page mid-scroll so segment 0 has a non-zero seek target. */
export function stubScrollY(y: number) {
  Object.defineProperty(window, 'scrollY', { configurable: true, value: y });
}

export const inside = (els: HTMLMediaElement[], container: HTMLElement) =>
  els.filter((v) => container.contains(v));

const originalCreateObjectURL = URL.createObjectURL;
const originalRevokeObjectURL = URL.revokeObjectURL;
const originalScreen = { width: window.screen.width, height: window.screen.height };

/** A touch device whose clips actually materialise as <video> elements. */
export function touchDeviceWithVideos(playResult: 'resolve' | 'reject' | 'no-promise') {
  vi.stubGlobal('requestAnimationFrame', () => 0);
  stubMatchMedia({ coarse: true, narrow: true, reduce: false });
  stubScreen(430, 932);
  stubTouchPoints(5);
  URL.createObjectURL = () => 'blob:scrub-engine-test';
  URL.revokeObjectURL = () => {};
  return trackPriming(playResult);
}

/** Registers the per-test teardown; every scrub-engine test file calls this once at top level. */
export function registerScrubCleanup() {
  afterEach(() => {
    // Unwind mounts BEFORE restoring globals: the disposer calls cancelAnimationFrame
    // and URL.revokeObjectURL, which several tests stub.
    disposers.splice(0).forEach((dispose) => dispose());
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    URL.createObjectURL = originalCreateObjectURL;
    URL.revokeObjectURL = originalRevokeObjectURL;
    stubScreen(originalScreen.width, originalScreen.height);
    stubTouchPoints(0);
    stubScrollY(0);
    document.body.replaceChildren();
  });
}
