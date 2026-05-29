import { afterEach, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

import '@testing-library/jest-dom/vitest';

// Unmount React trees and clear the jsdom body between tests so component
// renders never bleed into one another.
afterEach(() => {
  cleanup();
});

// Render motion elements as plain DOM and pass AnimatePresence children through
// synchronously. jsdom has no layout/WAAPI engine, so real exit animations never
// resolve — which would leave dismissed/closed elements mounted forever.
vi.mock('motion/react', async () => {
  const React = await import('react');
  const MOTION_PROPS = new Set([
    'initial',
    'animate',
    'exit',
    'transition',
    'variants',
    'whileHover',
    'whileTap',
    'whileFocus',
    'whileInView',
    'whileDrag',
    'layout',
    'layoutId',
    'drag',
    'dragConstraints',
    'custom',
    'onAnimationComplete',
    'viewport',
  ]);
  // Cache one stable component per tag — returning a fresh component on every
  // proxy access would remount the subtree each render and detach refs.
  const cache = new Map<string, React.ComponentType<Record<string, unknown>>>();
  const motion = new Proxy(
    {},
    {
      get: (_target, tag: string) => {
        if (!cache.has(tag)) {
          const Comp = React.forwardRef(({ children, ...props }: Record<string, unknown>, ref) => {
            const domProps: Record<string, unknown> = {};
            for (const [k, v] of Object.entries(props)) {
              if (!MOTION_PROPS.has(k)) domProps[k] = v;
            }
            return React.createElement(tag, { ref, ...domProps }, children as React.ReactNode);
          });
          Comp.displayName = `motion.${tag}`;
          cache.set(tag, Comp as React.ComponentType<Record<string, unknown>>);
        }
        return cache.get(tag);
      },
    }
  );
  return {
    motion,
    AnimatePresence: ({ children }: { children: React.ReactNode }) =>
      React.createElement(React.Fragment, null, children),
    useReducedMotion: () => false,
  };
});

// jsdom does not implement matchMedia; several components read it on mount.
if (!window.matchMedia) {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }) as unknown as MediaQueryList;
}

// jsdom lacks ResizeObserver/IntersectionObserver — stub them for layout hooks.
class StubObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}
if (!('ResizeObserver' in globalThis)) {
  globalThis.ResizeObserver = StubObserver as unknown as typeof ResizeObserver;
}
if (!('IntersectionObserver' in globalThis)) {
  globalThis.IntersectionObserver = StubObserver as unknown as typeof IntersectionObserver;
}

// scrollTo is referenced by a few routes/components; jsdom leaves it undefined.
if (!window.scrollTo) {
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
}

// jsdom does not implement scrollIntoView; dropdowns + chat views call it.
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = vi.fn();
}
