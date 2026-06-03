import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { NavPill } from './NavPill';

describe('NavPill', () => {
  it('renders a decorative pill carrying the layoutId class hooks', () => {
    const { container } = render(<NavPill layoutId="test-pill" />);
    const pill = container.firstChild as HTMLElement;
    expect(pill).toBeInTheDocument();
    // decorative: hidden from the a11y tree (active state lives on the row)
    expect(pill).toHaveAttribute('aria-hidden');
    // never intercepts clicks meant for the row above it
    expect(pill.className).toContain('pointer-events-none');
    expect(pill.className).toContain('absolute');
    expect(pill.className).toContain('inset-0');
  });

  it('merges a custom className onto the pill', () => {
    const { container } = render(<NavPill layoutId="test-pill" className="rounded-full" />);
    expect((container.firstChild as HTMLElement).className).toContain('rounded-full');
  });
});
