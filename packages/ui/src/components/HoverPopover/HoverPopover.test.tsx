/**
 * HoverPopover — structural tests.
 *
 * Strategy:
 *  - motion/react is globally shimmed in vitest.setup.ts — AnimatePresence
 *    passes children through synchronously, so the portalled panel is visible
 *    immediately after mouseenter without any async wait. The motion.div shim
 *    uses React.forwardRef so panelRef (ref={panelRef}) attaches correctly to
 *    the real DOM element.
 *  - createPortal renders into document.body; screen queries search the full
 *    document so role="tooltip" is always reachable.
 *  - getBoundingClientRect returns zeros in jsdom — the component detects a
 *    null rect and sets `display:none` on the panel until a non-zero rect is
 *    available. We stub getBoundingClientRect on the wrapper element to return
 *    a realistic rect {top:200,bottom:220,left:100,right:300} so panelStyle
 *    resolves to `position:fixed` with real coordinates and the paddingBottom
 *    branch exercises correctly.
 *  - Hover keep-open/close is now GEOMETRY-BASED, not mouseenter/leave-based.
 *    While open, the component attaches a `document` `pointermove` listener.
 *    It reads the trigger rect (wrapperRef) and panel rect (panelRef), inflates
 *    each by PAD=8 px, and if the pointer is inside EITHER inflated rect it
 *    cancels any pending close timer; otherwise it schedules close after
 *    closeDelay ms (default 120 ms). A `document` `pointerleave` also schedules
 *    close when the cursor exits the window.
 *  - jsdom pointer event coordinate nuance: jsdom's getBoundingClientRect for
 *    the PANEL element (panelRef) returns all-zeros. With PAD=8, any pointer
 *    dispatched at (0,0) falls inside the inflated zero rect (−8..8 on both
 *    axes) and keeps the panel open. To trigger a scheduled close, dispatch a
 *    pointermove at coordinates well outside BOTH rects (e.g. 9999,9999).
 *    closeDelay=120 ms, so fake timers are required to observe the close.
 *  - We NEVER assert on gradient CSS values (jsdom cssstyle gotcha).
 *    Assertions are structural: role, className tokens, inline padding values.
 *
 * Covers:
 *  - Panel not in document before open.
 *  - Opens on mouseenter on the wrapper.
 *  - Opens on focus on the wrapper.
 *  - Esc closes the panel immediately.
 *  - Blur on wrapper schedules close (after closeDelay).
 *  - placement="top": panel has inline paddingBottom.
 *  - placement="bottom": panel has inline paddingTop (not paddingBottom).
 *  - contentClassName on inner div, not on the panel element itself.
 *  - Content text accessible inside the element bearing contentClassName.
 *  - aria-expanded false→true transition.
 *  - ariaLabel prop wired to panel.
 *  - Geometry close: pointermove far outside both rects → schedules close.
 *  - Geometry keep-open: pointermove inside the trigger rect → no close.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';

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
    closeDelay?: number;
  } = {}
) {
  const {
    placement = 'top',
    contentClassName = 'popover-content',
    ariaLabel = 'Activity popover',
    children = <span>Panel content</span>,
    closeDelay,
  } = overrides;

  const result = render(
    <HoverPopover
      trigger={<button type="button">Trigger</button>}
      placement={placement}
      contentClassName={contentClassName}
      ariaLabel={ariaLabel}
      {...(closeDelay !== undefined ? { closeDelay } : {})}
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

afterEach(() => {
  // Ensure fake timers are always cleaned up even if a test throws.
  vi.useRealTimers();
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

  it('panel appears in the document after focus on the wrapper', () => {
    const { container } = renderPopover();
    stubRect(container);
    const wrapper = container.firstChild as HTMLElement;
    fireEvent.focus(wrapper);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();
  });

  it('pressing Escape closes the panel immediately', () => {
    const { container } = renderPopover();
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    const wrapper = container.firstChild as HTMLElement;
    fireEvent.keyDown(wrapper, { key: 'Escape' });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('blur on the wrapper schedules close — panel is gone after closeDelay', () => {
    vi.useFakeTimers();
    const { container } = renderPopover({ closeDelay: 120 });
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    const wrapper = container.firstChild as HTMLElement;
    fireEvent.blur(wrapper);

    // Still present before the delay expires.
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    // Gone after the delay.
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();

    vi.useRealTimers();
  });
});

// ── geometry-based hover tracking ────────────────────────────────────────────

describe('HoverPopover — geometry-based hover tracking', () => {
  it('pointermove far outside both rects schedules close — panel gone after closeDelay', () => {
    vi.useFakeTimers();
    const { container } = renderPopover({ closeDelay: 120 });
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    // jsdom does not expose PointerEvent — use MouseEvent with type 'pointermove'.
    // The component listener reads e.clientX/clientY which MouseEvent provides.
    // Dispatch at (9999,9999): outside the stubbed trigger rect {top:200,
    // bottom:220,left:100,right:300} and outside the zero panel rect even
    // with PAD=8 (−8..8 on both axes).
    document.dispatchEvent(
      new MouseEvent('pointermove', {
        bubbles: true,
        cancelable: true,
        clientX: 9999,
        clientY: 9999,
      })
    );

    // Still present before closeDelay expires.
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    // Gone after closeDelay passes.
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();

    vi.useRealTimers();
  });

  it('pointermove inside the trigger rect cancels close — panel stays open after closeDelay', () => {
    vi.useFakeTimers();
    const { container } = renderPopover({ closeDelay: 120 });
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    // Dispatch inside the stubbed wrapper rect {top:200,bottom:220,left:100,right:300}.
    // clientX:150, clientY:210 is well within the rect (no PAD needed).
    document.dispatchEvent(
      new MouseEvent('pointermove', { bubbles: true, cancelable: true, clientX: 150, clientY: 210 })
    );

    // Advance well past closeDelay — panel must still be present.
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    vi.useRealTimers();
  });

  it('pointerleave on document schedules close — panel gone after closeDelay', () => {
    vi.useFakeTimers();
    const { container } = renderPopover({ closeDelay: 120 });
    openPopover(container);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    // onPointerLeave is NOT throttled, so no rAF flush needed.
    document.dispatchEvent(new MouseEvent('pointerleave', { bubbles: true }));

    // Still present before the delay expires.
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(screen.getByRole('tooltip')).toBeInTheDocument();

    // Gone after closeDelay passes.
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();

    vi.useRealTimers();
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
