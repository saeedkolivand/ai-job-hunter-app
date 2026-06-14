/**
 * HoverPopover — structural tests (feat/accent-gradients).
 *
 * Strategy:
 *  - motion/react is globally shimmed in vitest.setup.ts — AnimatePresence
 *    passes children through synchronously, so the portalled panel is visible
 *    immediately after mouseenter without any async wait.
 *  - createPortal renders into document.body; screen queries search the full
 *    document so role="tooltip" is always reachable.
 *  - getBoundingClientRect returns zeros in jsdom — the component detects a
 *    null rect and sets `display:none` on the panel until a non-zero rect is
 *    available. We stub getBoundingClientRect on the wrapper element to return
 *    a realistic rect so panelStyle resolves to `position:fixed` with real
 *    coordinates and the paddingBottom branch exercises correctly.
 *  - We NEVER assert on gradient CSS values (jsdom cssstyle gotcha).
 *    Assertions are structural: role, className tokens, inline paddingBottom.
 *
 * Covers (placement="top"):
 *  - Trigger open via mouseenter on the wrapper.
 *  - Portalled panel gets role="tooltip".
 *  - Panel element itself does NOT carry contentClassName (it lives on the
 *    inner wrapper div).
 *  - Inner wrapper div carries contentClassName.
 *  - Panel has inline paddingBottom set (the 8 px pointer-bridge gap).
 *  - Content text is accessible inside the element bearing contentClassName.
 *  - Esc closes the panel.
 *  - placement="bottom": panel has inline paddingTop instead of paddingBottom.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { HoverPopover } from './HoverPopover';

// ── helpers ───────────────────────────────────────────────────────────────────

/** Stub getBoundingClientRect on whatever element is returned by wrapperRef. */
function stubRect(container: HTMLElement, rect: Partial<DOMRect> = {}) {
  const defaultRect: DOMRect = {
    top: 200,
    bottom: 220,
    left: 100,
    right: 300,
    width: 200,
    height: 20,
    x: 100,
    y: 200,
    toJSON: () => ({}),
  };
  const el = container.firstChild as HTMLElement;
  vi.spyOn(el, 'getBoundingClientRect').mockReturnValue({ ...defaultRect, ...rect });
}

function renderPopover(
  overrides: {
    placement?: 'top' | 'bottom';
    contentClassName?: string;
    ariaLabel?: string;
    children?: React.ReactNode;
  } = {}
) {
  const {
    placement = 'top',
    contentClassName = 'popover-content',
    ariaLabel = 'Activity popover',
    children = <span>Panel content</span>,
  } = overrides;

  const result = render(
    <HoverPopover
      trigger={<button type="button">Trigger</button>}
      placement={placement}
      contentClassName={contentClassName}
      ariaLabel={ariaLabel}
    >
      {children}
    </HoverPopover>
  );

  return result;
}

/** Open the popover by firing mouseenter on the outermost wrapper div. */
function openPopover(container: HTMLElement) {
  stubRect(container);
  const wrapper = container.firstChild as HTMLElement;
  fireEvent.mouseEnter(wrapper);
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.restoreAllMocks();
});

// ── open / close mechanics ────────────────────────────────────────────────────

describe('HoverPopover — open/close', () => {
  it('panel is not in the document before mouseenter', () => {
    renderPopover();
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('panel appears in the document after mouseenter on the wrapper', () => {
    const { container } = renderPopover();
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();
  });

  it('pressing Escape closes the panel', () => {
    const { container } = renderPopover();
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    const wrapper = container.firstChild as HTMLElement;
    fireEvent.keyDown(wrapper, { key: 'Escape' });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });
});

// ── placement="top" structural assertions ─────────────────────────────────────

describe('HoverPopover — placement="top" structure', () => {
  it('panel has inline paddingBottom (the 8px pointer-bridge gap)', () => {
    const { container } = renderPopover({ placement: 'top' });
    openPopover(container);

    const panel = screen.getByRole('tooltip');
    // paddingBottom is set as a numeric px value via React inline style.
    expect(panel).toHaveStyle({ paddingBottom: '8px' });
  });

  it('panel element itself does NOT carry contentClassName', () => {
    const { container } = renderPopover({ placement: 'top', contentClassName: 'my-content' });
    openPopover(container);

    const panel = screen.getByRole('tooltip');
    expect(panel.className).not.toContain('my-content');
  });

  it('inner wrapper div carries contentClassName', () => {
    const { container } = renderPopover({ placement: 'top', contentClassName: 'my-content' });
    openPopover(container);

    // The inner <div> is the direct child of role="tooltip".
    const panel = screen.getByRole('tooltip');
    const inner = panel.firstElementChild as HTMLElement;
    expect(inner).not.toBeNull();
    expect(inner.className).toContain('my-content');
  });

  it('content text is accessible inside the element with contentClassName', () => {
    const { container } = renderPopover({
      placement: 'top',
      contentClassName: 'my-content',
      children: <span>Hello world</span>,
    });
    openPopover(container);

    const inner = screen.getByRole('tooltip').firstElementChild as HTMLElement;
    expect(inner).toHaveTextContent('Hello world');
  });
});

// ── placement="bottom" structure ──────────────────────────────────────────────

describe('HoverPopover — placement="bottom" structure', () => {
  it('panel has inline paddingTop (not paddingBottom) for bottom placement', () => {
    const { container } = renderPopover({ placement: 'bottom' });
    openPopover(container);

    const panel = screen.getByRole('tooltip');
    expect(panel).toHaveStyle({ paddingTop: '8px' });
    // paddingBottom must NOT be set for bottom placement
    const styleAttr = panel.getAttribute('style') ?? '';
    expect(styleAttr).not.toMatch(/padding-bottom\s*:\s*8/);
  });

  it('panel element itself does NOT carry contentClassName for bottom placement', () => {
    const { container } = renderPopover({
      placement: 'bottom',
      contentClassName: 'bottom-content',
    });
    openPopover(container);

    const panel = screen.getByRole('tooltip');
    expect(panel.className).not.toContain('bottom-content');
  });
});

// ── aria wiring ───────────────────────────────────────────────────────────────

describe('HoverPopover — aria wiring', () => {
  it('wrapper has aria-expanded=false before open', () => {
    const { container } = renderPopover();
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper).toHaveAttribute('aria-expanded', 'false');
  });

  it('wrapper has aria-expanded=true while panel is open', () => {
    const { container } = renderPopover();
    openPopover(container);
    const wrapper = container.firstChild as HTMLElement;
    expect(wrapper).toHaveAttribute('aria-expanded', 'true');
  });

  it('panel carries the ariaLabel supplied via prop', () => {
    const { container } = renderPopover({ ariaLabel: 'Worker activity' });
    openPopover(container);
    expect(screen.getByRole('tooltip', { name: 'Worker activity' })).toBeInTheDocument();
  });
});
