// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';

import {
  flush,
  inside,
  mount,
  registerScrubCleanup,
  stubFetch,
  stubMatchMedia,
  stubScreen,
  stubTouchPoints,
  touchDeviceWithVideos,
  trackPriming,
} from './test-support';

registerScrubCleanup();

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
