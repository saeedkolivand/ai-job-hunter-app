import { createElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { useMouseParallax } from './use-mouse-parallax';

// Attaches the ref to a real DOM node so the hook can write CSS vars to it.
function Probe() {
  const ref = useMouseParallax<HTMLDivElement>();
  return createElement('div', { ref, 'data-testid': 'bg' });
}

describe('useMouseParallax', () => {
  afterEach(() => vi.restoreAllMocks());

  it('writes normalized pointer offset to CSS vars on the ref element', () => {
    // rAF runs the update; run it synchronously for the test.
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((cb: FrameRequestCallback) => {
      cb(0);
      return 1;
    });
    const { getByTestId } = render(createElement(Probe));
    const el = getByTestId('bg');

    act(() => {
      window.innerWidth = 1000;
      window.innerHeight = 1000;
      // jsdom lacks PointerEvent; a MouseEvent with the same type drives the
      // 'pointermove' listener and carries clientX/clientY.
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: 1000, clientY: 0 }));
    });

    expect(el.style.getPropertyValue('--parallax-x')).toBe('1.0000');
    expect(el.style.getPropertyValue('--parallax-y')).toBe('-1.0000');
  });

  it('cleans up the listener on unmount', () => {
    const remove = vi.spyOn(window, 'removeEventListener');
    const { unmount } = render(createElement(Probe));
    unmount();
    expect(remove).toHaveBeenCalledWith('pointermove', expect.any(Function));
  });
});
