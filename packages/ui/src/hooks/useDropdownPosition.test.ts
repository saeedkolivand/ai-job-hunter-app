import { useRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useDropdownPosition } from './useDropdownPosition';

// Helper: build a fake DOMRect
function fakeRect(overrides: Partial<DOMRect> = {}): DOMRect {
  const r = {
    left: 100,
    right: 300,
    top: 400,
    bottom: 450,
    width: 200,
    height: 50,
    x: 100,
    y: 400,
    toJSON() {
      return this;
    },
    ...overrides,
  } as DOMRect;
  return r;
}

// Spy on getBoundingClientRect and window dimensions before each test
function setupWindow(
  rect: Partial<DOMRect>,
  { innerWidth = 1280, innerHeight = 800 }: { innerWidth?: number; innerHeight?: number } = {}
) {
  vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(innerWidth);
  vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(innerHeight);
  const getBCR = vi.fn().mockReturnValue(fakeRect(rect));
  return { getBCR };
}

describe('useDropdownPosition', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('normal case: left matches rect.left, minWidth matches rect.width when narrower than maxWidth, width is max-content', () => {
    // rect.width=200, innerWidth=1280 → maxWidth=min(420,1264)=420; 200 < 420 so minWidth=200
    // rect.left=100, 100+420=520 < 1272 → no right-clamp → left=100
    const { getBCR } = setupWindow({ left: 100, right: 300, width: 200, bottom: 450, top: 400 });

    const { result } = renderHook(() => {
      const ref = useRef<HTMLButtonElement>(null);
      // Attach getBCR to the ref's current
      Object.defineProperty(ref, 'current', {
        get: () => ({ getBoundingClientRect: getBCR }),
        configurable: true,
      });
      return useDropdownPosition(true, ref);
    });

    // Trigger measure by activating an event — the hook calls measure() inside useEffect
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });

    const style = result.current.dropdownStyle;
    expect(style).not.toHaveProperty('display', 'none');
    expect((style as Record<string, unknown>).width).toBe('max-content');
    expect((style as Record<string, unknown>).minWidth).toBe(200);
    expect((style as Record<string, unknown>).left).toBe(100);
  });

  it('trigger wider than maxWidth: minWidth is capped at maxWidth', () => {
    // rect.width=500 > maxWidth=min(420,1264)=420 → minWidth=420
    const { getBCR } = setupWindow({ left: 0, right: 500, width: 500, bottom: 450, top: 400 });

    const { result } = renderHook(() => {
      const ref = useRef<HTMLButtonElement>(null);
      Object.defineProperty(ref, 'current', {
        get: () => ({ getBoundingClientRect: getBCR }),
        configurable: true,
      });
      return useDropdownPosition(true, ref);
    });

    act(() => {
      window.dispatchEvent(new Event('resize'));
    });

    const style = result.current.dropdownStyle as Record<string, unknown>;
    expect(style.minWidth).toBe(420);
  });

  it('right-edge overflow: left is clamped so the panel stays in viewport', () => {
    // innerWidth=400, maxWidth=min(420,384)=384
    // rect.left=350, rect.right=400, width=50
    // rect.left + maxWidth = 350+384=734 > 400-8=392 → clamp: max(8, rect.right - maxWidth) = max(8, 400-384) = max(8,16) = 16
    const { getBCR } = setupWindow(
      { left: 350, right: 400, width: 50, bottom: 450, top: 400 },
      { innerWidth: 400 }
    );

    const { result } = renderHook(() => {
      const ref = useRef<HTMLButtonElement>(null);
      Object.defineProperty(ref, 'current', {
        get: () => ({ getBoundingClientRect: getBCR }),
        configurable: true,
      });
      return useDropdownPosition(true, ref);
    });

    act(() => {
      window.dispatchEvent(new Event('resize'));
    });

    const style = result.current.dropdownStyle as Record<string, unknown>;
    // left = max(8, 400 - 384) = max(8, 16) = 16
    expect(style.left).toBe(16);
    // Must not overflow: left + maxWidth <= innerWidth - 8 is not guaranteed to be perfect
    // but the hook ensures left >= 8
    expect((style.left as number) >= 8).toBe(true);
  });

  it('drop-up: little space below triggers dropUp=true and style uses bottom instead of top', () => {
    // innerHeight=500, rect.bottom=450 → spaceBelow=50 < 220 → dropUp=true
    const { getBCR } = setupWindow(
      { left: 100, right: 300, width: 200, bottom: 450, top: 400 },
      { innerHeight: 500 }
    );

    const { result } = renderHook(() => {
      const ref = useRef<HTMLButtonElement>(null);
      Object.defineProperty(ref, 'current', {
        get: () => ({ getBoundingClientRect: getBCR }),
        configurable: true,
      });
      return useDropdownPosition(true, ref);
    });

    act(() => {
      window.dispatchEvent(new Event('resize'));
    });

    expect(result.current.dropUp).toBe(true);
    const style = result.current.dropdownStyle as Record<string, unknown>;
    expect(style).toHaveProperty('bottom');
    expect(style).not.toHaveProperty('top');
  });

  it('drop-down (enough space below): dropUp=false and style uses top not bottom', () => {
    // innerHeight=800, rect.bottom=100 → spaceBelow=700 >= 220 → dropUp=false
    const { getBCR } = setupWindow(
      { left: 100, right: 300, width: 200, bottom: 100, top: 50 },
      { innerHeight: 800 }
    );

    const { result } = renderHook(() => {
      const ref = useRef<HTMLButtonElement>(null);
      Object.defineProperty(ref, 'current', {
        get: () => ({ getBoundingClientRect: getBCR }),
        configurable: true,
      });
      return useDropdownPosition(true, ref);
    });

    act(() => {
      window.dispatchEvent(new Event('resize'));
    });

    expect(result.current.dropUp).toBe(false);
    const style = result.current.dropdownStyle as Record<string, unknown>;
    expect(style).toHaveProperty('top');
    expect(style).not.toHaveProperty('bottom');
  });
});
