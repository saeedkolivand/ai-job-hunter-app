import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Image } from './index';

const SRC = 'https://example.com/a.png';
const SRC2 = 'https://example.com/b.png';
const FALLBACK = 'https://example.com/fallback.png';

describe('Image', () => {
  it('renders an img with the src and alt', () => {
    render(<Image src={SRC} alt="hello" />);
    const img = screen.getByAltText('hello');
    expect(img).toHaveAttribute('src', SRC);
  });

  it('swaps to the fallback src on load error and calls onError', () => {
    const onError = vi.fn();
    render(<Image src={SRC} alt="x" fallback={FALLBACK} onError={onError} />);
    fireEvent.error(screen.getByAltText('x'));
    expect(screen.getByAltText('x')).toHaveAttribute('src', FALLBACK);
    expect(onError).toHaveBeenCalledTimes(1);
  });

  it('shows a preview mask by default and opens the lightbox on click', async () => {
    const user = userEvent.setup();
    render(<Image src={SRC} alt="x" />);

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Preview image' }));

    const dialog = screen.getByRole('dialog');
    expect(dialog).toBeInTheDocument();
    // Toolbar actions are present.
    expect(screen.getByRole('button', { name: 'Zoom in' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Rotate right' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('hides the mask and disables preview when preview={false}', () => {
    render(<Image src={SRC} alt="x" preview={false} />);
    expect(screen.queryByRole('button', { name: 'Preview image' })).not.toBeInTheDocument();
  });

  it('uses a custom preview src when configured', async () => {
    const user = userEvent.setup();
    render(<Image src={SRC} alt="x" preview={{ src: SRC2 }} />);
    await user.click(screen.getByRole('button', { name: 'Preview image' }));
    // The lightbox shows the custom preview src, not the thumbnail src.
    const previewImg = screen.getByRole('dialog').querySelector('img');
    expect(previewImg).toHaveAttribute('src', SRC2);
  });
});

describe('Image.PreviewGroup', () => {
  it('shares one lightbox with prev/next across child images', async () => {
    const user = userEvent.setup();
    render(
      <Image.PreviewGroup>
        <Image src={SRC} alt="first" />
        <Image src={SRC2} alt="second" />
      </Image.PreviewGroup>
    );

    // Open from the first child.
    const masks = screen.getAllByRole('button', { name: 'Preview image' });
    expect(masks).toHaveLength(2);
    const [first] = masks;
    if (!first) throw new Error('expected a preview mask');
    await user.click(first);

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText('1 / 2')).toBeInTheDocument();

    // Navigate to the next image.
    await user.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.getByText('2 / 2')).toBeInTheDocument();
  });
});
