import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ImagePreview, type ImagePreviewProps } from './ImagePreview';

const SRC_A = 'https://example.com/a.png';
const SRC_B = 'https://example.com/b.png';
const SRC_C = 'https://example.com/c.png';

// Controlled harness so state changes are reflected in re-renders.
function Harness({
  items = [SRC_A],
  initialIndex = 0,
  initialOpen = true,
  ...rest
}: Partial<Omit<ImagePreviewProps, 'index' | 'open' | 'onIndexChange' | 'onOpenChange'>> & {
  initialIndex?: number;
  initialOpen?: boolean;
}) {
  const [index, setIndex] = useState(initialIndex);
  const [open, setOpen] = useState(initialOpen);
  return (
    <ImagePreview
      items={items}
      index={index}
      open={open}
      onIndexChange={setIndex}
      onOpenChange={setOpen}
      {...rest}
    />
  );
}

// Helper: read the transform style on the preview <img>.
function imgTransform() {
  const img = screen.getByRole('dialog').querySelector('img');
  if (!img) throw new Error('preview img not found');
  return img.style.transform;
}

describe('ImagePreview — render / open-close', () => {
  it('renders nothing when open=false', () => {
    render(<Harness initialOpen={false} />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('renders nothing when items is empty', () => {
    render(<Harness items={[]} />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('renders the lightbox with the correct src when open', () => {
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    expect(img).toHaveAttribute('src', SRC_A);
  });

  it('closes when the Close button is clicked', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('closes when the backdrop is clicked', async () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('dialog'));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('does not close when the image itself is clicked (stopPropagation)', async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const img = screen.getByRole('dialog').querySelector('img');
    if (!img) throw new Error('img not found');
    await user.click(img);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });
});

describe('ImagePreview — keyboard navigation', () => {
  it('closes on Escape', async () => {
    render(<Harness />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('does not respond to arrow keys when there is only one item', () => {
    const onIndexChange = vi.fn();
    const { unmount } = render(
      <ImagePreview
        items={[SRC_A]}
        index={0}
        open
        onIndexChange={onIndexChange}
        onOpenChange={vi.fn()}
      />
    );
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(onIndexChange).not.toHaveBeenCalled();
    unmount();
  });

  it('navigates to the previous item on ArrowLeft', () => {
    const onIndexChange = vi.fn();
    const { unmount } = render(
      <ImagePreview
        items={[SRC_A, SRC_B, SRC_C]}
        index={1}
        open
        onIndexChange={onIndexChange}
        onOpenChange={vi.fn()}
      />
    );
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    // index 1 - 1 = 0
    expect(onIndexChange).toHaveBeenCalledWith(0);
    unmount();
  });

  it('navigates to the next item on ArrowRight', () => {
    const onIndexChange = vi.fn();
    const { unmount } = render(
      <ImagePreview
        items={[SRC_A, SRC_B, SRC_C]}
        index={1}
        open
        onIndexChange={onIndexChange}
        onOpenChange={vi.fn()}
      />
    );
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    // index 1 + 1 = 2
    expect(onIndexChange).toHaveBeenCalledWith(2);
    unmount();
  });

  it('wraps ArrowLeft from index 0 to the last item', () => {
    const onIndexChange = vi.fn();
    const { unmount } = render(
      <ImagePreview
        items={[SRC_A, SRC_B, SRC_C]}
        index={0}
        open
        onIndexChange={onIndexChange}
        onOpenChange={vi.fn()}
      />
    );
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    expect(onIndexChange).toHaveBeenCalledWith(2);
    unmount();
  });

  it('wraps ArrowRight from the last item to index 0', () => {
    const onIndexChange = vi.fn();
    const { unmount } = render(
      <ImagePreview
        items={[SRC_A, SRC_B, SRC_C]}
        index={2}
        open
        onIndexChange={onIndexChange}
        onOpenChange={vi.fn()}
      />
    );
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(onIndexChange).toHaveBeenCalledWith(0);
    unmount();
  });

  it('removes the keydown listener when closed', () => {
    const onOpenChange = vi.fn();
    const { rerender } = render(
      <ImagePreview
        items={[SRC_A]}
        index={0}
        open
        onIndexChange={vi.fn()}
        onOpenChange={onOpenChange}
      />
    );
    // Close via re-render (simulates parent responding to onOpenChange).
    rerender(
      <ImagePreview
        items={[SRC_A]}
        index={0}
        open={false}
        onIndexChange={vi.fn()}
        onOpenChange={onOpenChange}
      />
    );
    // Escape should NOT fire onOpenChange again now that it's closed.
    onOpenChange.mockClear();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onOpenChange).not.toHaveBeenCalled();
  });
});

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
    const { rerender } = render(
      <ImagePreview
        items={[SRC_A, SRC_B]}
        index={0}
        open
        onIndexChange={vi.fn()}
        onOpenChange={vi.fn()}
      />
    );
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

describe('ImagePreview — multi-image navigation buttons', () => {
  it('does not render prev/next buttons for a single image', () => {
    render(<Harness items={[SRC_A]} />);
    expect(screen.queryByRole('button', { name: 'Previous' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Next' })).not.toBeInTheDocument();
  });

  it('renders prev/next buttons and counter for multiple images', () => {
    render(<Harness items={[SRC_A, SRC_B, SRC_C]} initialIndex={0} />);
    expect(screen.getByRole('button', { name: 'Previous' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Next' })).toBeInTheDocument();
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
  });

  it('Next button advances the index and updates the counter', async () => {
    const user = userEvent.setup();
    render(<Harness items={[SRC_A, SRC_B, SRC_C]} initialIndex={0} />);
    await user.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.getByText('2 / 3')).toBeInTheDocument();
  });

  it('Previous button decrements the index', async () => {
    const user = userEvent.setup();
    render(<Harness items={[SRC_A, SRC_B, SRC_C]} initialIndex={1} />);
    await user.click(screen.getByRole('button', { name: 'Previous' }));
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
  });

  it('Next wraps from last to first', async () => {
    const user = userEvent.setup();
    render(<Harness items={[SRC_A, SRC_B, SRC_C]} initialIndex={2} />);
    await user.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.getByText('1 / 3')).toBeInTheDocument();
  });

  it('Previous wraps from first to last', async () => {
    const user = userEvent.setup();
    render(<Harness items={[SRC_A, SRC_B, SRC_C]} initialIndex={0} />);
    await user.click(screen.getByRole('button', { name: 'Previous' }));
    expect(screen.getByText('3 / 3')).toBeInTheDocument();
  });
});

describe('ImagePreview — imageRender / toolbarRender slots', () => {
  it('imageRender receives the live transform and replaces the image node', async () => {
    const user = userEvent.setup();
    const imageRender = vi.fn(
      (_node: React.ReactNode, info: { transform: { scale: number }; current: number }) => (
        <div
          data-testid="custom-image"
          data-scale={info.transform.scale}
          data-current={info.current}
        />
      )
    );
    render(<Harness imageRender={imageRender} />);
    expect(screen.getByTestId('custom-image')).toHaveAttribute('data-scale', '1');

    await user.click(screen.getByRole('button', { name: 'Zoom in' }));
    expect(screen.getByTestId('custom-image')).toHaveAttribute('data-scale', '1.5');
  });

  it('toolbarRender receives transform + total and replaces the toolbar', async () => {
    const toolbarRender = vi.fn(
      (
        _node: React.ReactNode,
        info: { transform: { rotate: number }; current: number; total: number }
      ) => (
        <div
          data-testid="custom-toolbar"
          data-rotate={info.transform.rotate}
          data-total={info.total}
        />
      )
    );
    render(<Harness items={[SRC_A, SRC_B]} toolbarRender={toolbarRender} />);
    expect(screen.getByTestId('custom-toolbar')).toHaveAttribute('data-total', '2');
    expect(screen.getByTestId('custom-toolbar')).toHaveAttribute('data-rotate', '0');

    // No default toolbar buttons — they were replaced.
    expect(screen.queryByRole('button', { name: 'Zoom in' })).not.toBeInTheDocument();
  });
});

describe('ImagePreview — body scroll lock', () => {
  it('locks body overflow while open and restores it on close', () => {
    document.body.style.overflow = 'auto';
    const { unmount } = render(
      <ImagePreview items={[SRC_A]} index={0} open onIndexChange={vi.fn()} onOpenChange={vi.fn()} />
    );
    expect(document.body.style.overflow).toBe('hidden');
    unmount();
    expect(document.body.style.overflow).toBe('auto');
  });
});
