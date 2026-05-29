import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { useMouseParallax } from './use-mouse-parallax';

describe('useMouseParallax', () => {
  afterEach(() => vi.restoreAllMocks());

  it('starts centred with 50%/50% CSS vars', () => {
    const { result } = renderHook(() => useMouseParallax());
    expect(result.current.x).toBe(0);
    expect(result.current.y).toBe(0);
    expect(result.current.mouseVars).toEqual({ '--mx': '50%', '--my': '50%' });
  });

  it('updates normalized position on pointer movement', async () => {
    // rAF runs the update; run it synchronously for the test.
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb: FrameRequestCallback) => {
      cb(0);
      return 1;
    });
    const { result } = renderHook(() => useMouseParallax());

    act(() => {
      window.innerWidth = 1000;
      window.innerHeight = 1000;
      // jsdom lacks PointerEvent; a MouseEvent with the same type drives the
      // 'pointermove' listener and carries clientX/clientY.
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 1000, clientY: 0 }));
    });

    await waitFor(() => {
      expect(result.current.x).toBeCloseTo(1);
      expect(result.current.y).toBeCloseTo(-1);
    });
  });

  it('cleans up the listener on unmount', () => {
    const remove = vi.spyOn(window, 'removeEventListener');
    const { unmount } = renderHook(() => useMouseParallax());
    unmount();
    expect(remove).toHaveBeenCalledWith('pointermove', expect.any(Function));
  });
});
