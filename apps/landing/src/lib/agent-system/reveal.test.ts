import { afterEach, describe, expect, it, vi } from 'vitest';

import { COUNT_UP_DURATION_MS } from './constants';
import { countUp, easeOutCubic, sceneProgress } from './reveal';

describe('easeOutCubic', () => {
  it('is 0 at p=0 and 1 at p=1', () => {
    expect(easeOutCubic(0)).toBe(0);
    expect(easeOutCubic(1)).toBe(1);
  });

  it('is monotonically increasing and decelerating across [0,1]', () => {
    const samples = [0, 0.2, 0.4, 0.6, 0.8, 1].map(easeOutCubic);
    const deltas: number[] = [];
    for (let i = 1; i < samples.length; i += 1) {
      const prev = samples[i - 1] ?? 0;
      const cur = samples[i] ?? 0;
      expect(cur).toBeGreaterThan(prev); // monotonic
      deltas.push(cur - prev);
    }
    // decelerating: each step's gain is smaller than the previous step's
    for (let i = 1; i < deltas.length; i += 1) {
      const prev = deltas[i - 1] ?? 0;
      const cur = deltas[i] ?? 0;
      expect(cur).toBeLessThan(prev);
    }
  });

  it('is not clamped to [0,1] itself — callers clamp p before calling', () => {
    expect(easeOutCubic(2)).toBe(2); // 1 - (1-2)^3 = 1 - (-1) = 2
  });
});

describe('sceneProgress', () => {
  it.each([
    [1000, 500, 500, 0, 'far below the viewport (not yet reached) clamps to 0'],
    [-2000, 500, 500, 1, 'scrolled well past the scene clamps to 1'],
    [0, 500, 500, 0.5, 'top at the viewport edge with height === viewport is the midpoint'],
    [500, 500, 500, 0, 'top === viewportHeight is exactly the lower bound'],
    [-500, 500, 500, 1, 'top === -viewportHeight is exactly the upper bound'],
  ] as const)(
    'sceneProgress(top=%d, height=%d, viewportHeight=%d) === %d (%s)',
    (top, height, viewportHeight, expected, _why) => {
      expect(sceneProgress(top, height, viewportHeight)).toBe(expected);
    }
  );

  it('is NaN (not clamped) when height + viewportHeight is 0 — a 0/0 division', () => {
    // Documented current behavior: a zero-height scene in a zero-height viewport
    // never happens in practice, but the clamp comparisons (p < 0, p > 1) are both
    // false for NaN, so the raw NaN passes through unclamped.
    expect(Number.isNaN(sceneProgress(0, 0, 0))).toBe(true);
  });
});

describe('countUp', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('reduced-motion: sets textContent to the final value synchronously, no window access', () => {
    const el = { textContent: null as string | null } as unknown as HTMLElement;
    // `window` is deliberately left unstubbed (undefined in this node-env test) to
    // prove the reduced-motion branch never touches it.
    countUp(el, 247, true);
    expect(el.textContent).toBe('247');
  });

  it('reduced-motion with to=0 still sets "0"', () => {
    const el = { textContent: null as string | null } as unknown as HTMLElement;
    countUp(el, 0, true);
    expect(el.textContent).toBe('0');
  });

  it('animated: eases from 0 up to `to`, then snaps exactly to `to` on the final frame', () => {
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal('window', {
      requestAnimationFrame: (cb: FrameRequestCallback) => {
        frames.push(cb);
        return frames.length;
      },
    });
    const el = { textContent: null as string | null } as unknown as HTMLElement;

    countUp(el, 200, false);
    expect(el.textContent).toBe('0'); // synchronous initial paint
    expect(frames).toHaveLength(1);

    const runNextFrame = (t: number) => {
      const cb = frames.shift();
      if (!cb) throw new Error('expected a scheduled animation frame');
      cb(t);
    };

    const start = 1000;
    runNextFrame(start); // p = 0
    expect(el.textContent).toBe(String(Math.round(easeOutCubic(0) * 200)));
    expect(frames).toHaveLength(1); // reschedules itself

    runNextFrame(start + COUNT_UP_DURATION_MS / 2); // p = 0.5
    expect(el.textContent).toBe(String(Math.round(easeOutCubic(0.5) * 200)));
    expect(frames).toHaveLength(1);

    runNextFrame(start + COUNT_UP_DURATION_MS); // p = 1, done
    expect(el.textContent).toBe('200');
    expect(frames).toHaveLength(0); // no further frame scheduled
  });
});
