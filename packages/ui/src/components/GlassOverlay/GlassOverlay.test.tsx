import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { GlassOverlay } from './GlassOverlay';

describe('GlassOverlay', () => {
  it('renders a fixed backdrop', () => {
    const { container } = render(<GlassOverlay />);
    const el = container.firstChild as HTMLElement;
    expect(el.className).toContain('fixed');
    expect(el).toHaveAttribute('aria-hidden', 'true');
  });

  it('calls onClick when the backdrop is clicked', async () => {
    const onClick = vi.fn();
    const { container } = render(<GlassOverlay onClick={onClick} />);
    await userEvent.click(container.firstChild as HTMLElement);
    expect(onClick).toHaveBeenCalledOnce();
  });
});
