// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';

import {
  captureFrames,
  fakeMedia,
  flush,
  mount,
  registerScrubCleanup,
  stubFetch,
  stubMatchMedia,
  stubScreen,
  stubScrollY,
  stubTouchPoints,
  trackPriming,
} from './test-support';

registerScrubCleanup();

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
    // The threshold itself, from both sides — the rule is inclusive on the phone side.
    {
      name: 'touch device on the threshold',
      coarse: true,
      narrow: true,
      w: 500,
      h: 900,
      set: 'mobile',
    },
    {
      name: 'touch device one px over',
      coarse: true,
      narrow: true,
      w: 501,
      h: 900,
      set: 'desktop',
    },
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
