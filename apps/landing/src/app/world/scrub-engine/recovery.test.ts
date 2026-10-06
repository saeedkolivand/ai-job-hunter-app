// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';

import { mountScrollWorld } from '../scrub-engine';
import {
  captureFrames,
  disposers,
  fakeMedia,
  flush,
  mount,
  registerScrubCleanup,
  stubFetch,
  stubMatchMedia,
  stubScreen,
  stubScrollY,
  stubTouchPoints,
  touchDeviceWithVideos,
  trackPriming,
} from './test-support';

registerScrubCleanup();

// ---------------------------------------------------------------------------
// Teardown — upstream returns nothing and never unregisters, so a remount stacked
// a second listener set and a second unbounded rAF loop over the first.
// ---------------------------------------------------------------------------
describe('the disposer mountScrollWorld returns', () => {
  it('stops listening, stops the frame loop, releases the clips and empties the host', async () => {
    touchDeviceWithVideos('resolve');
    const revoked: string[] = [];
    URL.revokeObjectURL = (url: string) => void revoked.push(url);
    const urls = stubFetch('ok');
    const container = mount('teardown');
    await flush();

    const clips = container.querySelectorAll('video.sw-scene__video').length;
    expect(clips).toBeGreaterThan(0);
    const fetchesBefore = urls.length;

    const dispose = disposers.pop();
    if (!dispose) throw new Error('mountScrollWorld returned no disposer');
    dispose();

    expect(container.children).toHaveLength(0);
    expect(container.classList.contains('sw-root')).toBe(false);
    expect(revoked).toHaveLength(clips); // one blob released per live clip

    // Nothing the engine used to listen to can wake it up again.
    window.dispatchEvent(new Event('orientationchange'));
    window.dispatchEvent(new Event('resize'));
    window.dispatchEvent(new Event('pointerdown'));
    await flush();
    expect(urls).toHaveLength(fetchesBefore);

    // And it is idempotent — React can call an effect cleanup more than once.
    expect(() => dispose()).not.toThrow();
  });

  it('hands back a working disposer even when the config has no sections', () => {
    vi.stubGlobal('requestAnimationFrame', () => 0);
    stubMatchMedia({ coarse: false, narrow: false, reduce: false });
    const container = document.createElement('div');
    document.body.appendChild(container);
    const dispose = mountScrollWorld(container, { sections: [] });
    expect(() => dispose()).not.toThrow();
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
    // NOTE: window events reach EVERY live mount, not just this test's. afterEach now
    // disposes each mount, so tests no longer leak into one another — but a single
    // test that mounts twice still has two listeners on this event. The belt-and-
    // braces stays: `configFor` gives each mount unique asset paths and assertions
    // scope by prefix or `container.contains`. An assertion over an unscoped global
    // (total fetch count, prototype spy counts, a document-wide querySelectorAll)
    // would pick up the sibling mount — scope it the same way.
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
