import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ImagePreview } from './ImagePreview';
import { Harness, imgTransform, renderPreview, SRC_A, SRC_B, SRC_C } from './ImagePreview.support';

describe('ImagePreview — toolbar actions (zoom / rotate / flip / reset)', () => {
  it('zoom-in button multiplies scale by (1 + scaleStep)', async () => {
    const user = userEvent.setup();
    render(<Harness scaleStep={0.5} />);

    const before = imgTransform();
    // Default scale=1, scaleStep=0.5 → after zoom-in scale = 1.5
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    const after = imgTransform();

    expect(after).not.toEqual(before);
    expect(after).toContain('scale(1.5, 1.5)');
  });

  it('zoom-out button divides scale by (1 + scaleStep)', async () => {
    const user = userEvent.setup();
    render(<Harness scaleStep={0.5} />);

    // Zoom in first so we have room to zoom out.
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(imgTransform()).toContain('scale(1.5, 1.5)');

    await user.click(screen.getByRole('button', { name: 'Zoom out' }));
    // 1.5 / 1.5 = 1 (clamped to minScale=1)
    expect(imgTransform()).toContain('scale(1, 1)');
  });

  it('zoom-out is clamped to minScale', async () => {
    const user = userEvent.setup();
    render(<Harness scaleStep={0.5} minScale={1} />);
    // Already at minScale=1; zoom-out should stay at 1.
    await user.click(screen.getByRole('button', { name: 'Zoom out' }));
    expect(imgTransform()).toContain('scale(1, 1)');
  });

  it('zoom-in is clamped to maxScale', async () => {
    const user = userEvent.setup();
    render(<Harness scaleStep={0.5} maxScale={2} />);
    // Each zoom-in: 1 → 1.5 → 2.25 clamped to 2 → stays 2.
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(imgTransform()).toContain('scale(2, 2)');
  });

  it('rotate-right button adds 90° to rotation', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Rotate right' }));
    expect(imgTransform()).toContain('rotate(90deg)');
    await user.click(screen.getByRole('button', { name: 'Rotate right' }));
    expect(imgTransform()).toContain('rotate(180deg)');
  });

  it('rotate-left button subtracts 90° from rotation', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Rotate left' }));
    expect(imgTransform()).toContain('rotate(-90deg)');
  });

  it('flip-horizontal button negates x scale component', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    // No flip: scale(1, 1)
    expect(imgTransform()).toContain('scale(1, 1)');
    await user.click(screen.getByRole('button', { name: 'Flip horizontal' }));
    // flipX=true: sx = scale * -1 = -1
    expect(imgTransform()).toContain('scale(-1, 1)');
    // Toggle back
    await user.click(screen.getByRole('button', { name: 'Flip horizontal' }));
    expect(imgTransform()).toContain('scale(1, 1)');
  });

  it('flip-vertical button negates y scale component', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Flip vertical' }));
    expect(imgTransform()).toContain('scale(1, -1)');
    await user.click(screen.getByRole('button', { name: 'Flip vertical' }));
    expect(imgTransform()).toContain('scale(1, 1)');
  });

  it('flip H + V combination negates both components', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Flip horizontal' }));
    await user.click(screen.getByRole('button', { name: 'Flip vertical' }));
    expect(imgTransform()).toContain('scale(-1, -1)');
  });

  it('reset button restores identity transform', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    await user.click(screen.getByRole('button', { name: 'Rotate right' }));
    await user.click(screen.getByRole('button', { name: 'Flip horizontal' }));
    // Verify we've moved away from identity.
    expect(imgTransform()).not.toContain('rotate(0deg)');

    await user.click(screen.getByRole('button', { name: 'Reset' }));
    expect(imgTransform()).toContain('translate3d(0px, 0px, 0)');
    expect(imgTransform()).toContain('rotate(0deg)');
    expect(imgTransform()).toContain('scale(1, 1)');
  });

  it('reset via opening a new item also restores identity', () => {
    const { rerender } = renderPreview({ items: [SRC_A, SRC_B] });
    // Simulate parent setting index=1 in response to Next click.
    rerender(
      <ImagePreview
        items={[SRC_A, SRC_B]}
        index={1}
        open
        onIndexChange={vi.fn()}
        onOpenChange={vi.fn()}
      />
    );
    // Transform should be identity after item change.
    expect(imgTransform()).toContain('scale(1, 1)');
    expect(imgTransform()).toContain('rotate(0deg)');
  });
});

describe('ImagePreview — double-click zoom toggle', () => {
  it('zooms in on first double-click (scale 1 → 2)', () => {
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');
    fireEvent.doubleClick(img);
    expect(imgTransform()).toContain('scale(2, 2)');
  });

  it('resets to identity on second double-click when already zoomed', () => {
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');
    fireEvent.doubleClick(img);
    expect(imgTransform()).toContain('scale(2, 2)');
    fireEvent.doubleClick(img);
    expect(imgTransform()).toContain('scale(1, 1)');
    expect(imgTransform()).toContain('rotate(0deg)');
  });
});

// jsdom does not ship PointerEvent; polyfill it as MouseEvent so clientX/clientY
// are readable by the component's pointer handlers.
if (typeof globalThis.PointerEvent === 'undefined') {
  class PointerEventPolyfill extends MouseEvent {
    pointerId: number;
    constructor(type: string, init: PointerEventInit = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 0;
    }
  }
  globalThis.PointerEvent = PointerEventPolyfill as unknown as typeof PointerEvent;
}

// fireEvent wraps each dispatch in act(), which is what we need for pointer
// events that trigger setTransform state updates inside the component.
function ptrDown(el: Element, x: number, y: number) {
  fireEvent.pointerDown(el, { clientX: x, clientY: y, pointerId: 1 });
}
function ptrMove(el: Element, x: number, y: number) {
  fireEvent.pointerMove(el, { clientX: x, clientY: y, pointerId: 1 });
}
function ptrUp(el: Element) {
  fireEvent.pointerUp(el, { pointerId: 1 });
}

describe('ImagePreview — drag / pan', () => {
  it('panning is disabled when movable=false', () => {
    render(<Harness movable={false} />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');

    ptrDown(img, 100, 100);
    ptrMove(img, 150, 130);
    ptrUp(img);

    // x/y should remain 0 (pan not applied when movable=false).
    expect(imgTransform()).toContain('translate3d(0px, 0px, 0)');
  });

  it('drag-to-pan updates translate on pointer move', () => {
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');

    ptrDown(img, 100, 100);
    ptrMove(img, 160, 130);

    // dx=60, dy=30 relative to start
    expect(imgTransform()).toContain('translate3d(60px, 30px, 0)');
  });

  it('pan accumulates base position from previous drag', () => {
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');

    // First drag: move by (50, 20).
    ptrDown(img, 0, 0);
    ptrMove(img, 50, 20);
    ptrUp(img);

    // Second drag: base is now (50, 20); move another (30, 10).
    ptrDown(img, 0, 0);
    ptrMove(img, 30, 10);
    ptrUp(img);

    expect(imgTransform()).toContain('translate3d(80px, 30px, 0)');
  });

  it('pointer move before pointer down has no effect', () => {
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');
    ptrMove(img, 999, 999);
    expect(imgTransform()).toContain('translate3d(0px, 0px, 0)');
  });
});

describe('ImagePreview — keyboard pan', () => {
  it('arrow keys pan a zoomed image by a fixed step, in the same direction as a drag', async () => {
    const user = userEvent.setup();
    render(<Harness scaleStep={0.5} />);
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));

    fireEvent.keyDown(window, { key: 'ArrowRight' });
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    // 40px per press; ArrowRight/ArrowDown move the image right/down exactly as
    // a rightward/downward drag does.
    expect(imgTransform()).toContain('translate3d(40px, 40px, 0)');

    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    fireEvent.keyDown(window, { key: 'ArrowUp' });
    expect(imgTransform()).toContain('translate3d(0px, 0px, 0)');
  });

  it('does not pan at 1x — there is nothing outside the frame to reach', () => {
    render(<Harness />);
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    expect(imgTransform()).toContain('translate3d(0px, 0px, 0)');
  });

  it('does not pan when movable=false, even zoomed (parity with drag-to-pan)', async () => {
    const user = userEvent.setup();
    render(<Harness movable={false} scaleStep={0.5} />);
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(imgTransform()).toContain('translate3d(0px, 0px, 0)');
  });

  it('a zoomed multi-item preview pans instead of changing item', async () => {
    const user = userEvent.setup();
    const { onIndexChange } = renderPreview({
      items: [SRC_A, SRC_B, SRC_C],
      index: 1,
      scaleStep: 0.5,
    });
    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(onIndexChange).not.toHaveBeenCalled();
    expect(imgTransform()).toContain('translate3d(40px, 0px, 0)');
  });
});
