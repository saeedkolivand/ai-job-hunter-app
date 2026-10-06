import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ImagePreview } from './ImagePreview';
import { Harness, renderPreview, SRC_A, SRC_B, SRC_C } from './ImagePreview.support';

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
    const { onIndexChange, unmount } = renderPreview();
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    expect(onIndexChange).not.toHaveBeenCalled();
    unmount();
  });

  it.each([
    ['navigates to the previous item on ArrowLeft', 'ArrowLeft', 1, 0],
    ['navigates to the next item on ArrowRight', 'ArrowRight', 1, 2],
    ['wraps ArrowLeft from index 0 to the last item', 'ArrowLeft', 0, 2],
    ['wraps ArrowRight from the last item to index 0', 'ArrowRight', 2, 0],
  ])('%s', (_name, key, index, expected) => {
    const { onIndexChange, unmount } = renderPreview({ items: [SRC_A, SRC_B, SRC_C], index });
    fireEvent.keyDown(window, { key });
    expect(onIndexChange).toHaveBeenCalledWith(expected);
    unmount();
  });

  it('removes the keydown listener when closed', () => {
    const { rerender, onOpenChange } = renderPreview();
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
