import { useState } from 'react';
import { type Mock, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { ImagePreview, type ImagePreviewProps } from './ImagePreview';

export const SRC_A = 'https://example.com/a.png';
export const SRC_B = 'https://example.com/b.png';
export const SRC_C = 'https://example.com/c.png';

// Controlled harness so state changes are reflected in re-renders.
export function Harness({
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
export function imgTransform() {
  const img = screen.getByRole('dialog').querySelector('img');
  if (!img) throw new Error('preview img not found');
  return img.style.transform;
}

/** Uncontrolled render with spy callbacks, for tests that assert on the calls. */
export function renderPreview(
  props: Partial<ImagePreviewProps> = {}
): ReturnType<typeof render> & { onIndexChange: Mock; onOpenChange: Mock } {
  const onIndexChange = vi.fn();
  const onOpenChange = vi.fn();
  const utils = render(
    <ImagePreview
      items={[SRC_A]}
      index={0}
      open
      onIndexChange={onIndexChange}
      onOpenChange={onOpenChange}
      {...props}
    />
  );
  return { ...utils, onIndexChange, onOpenChange };
}
