import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/react';

import { CardSkeleton, RowSkeleton, Skeleton } from './LoadingSkeleton';

describe('LoadingSkeleton', () => {
  it('renders a single skeleton line with the animation class', () => {
    const { container } = render(<Skeleton className="h-4" />);
    const el = container.firstChild as HTMLElement;
    expect(el.className).toContain('animate-skeleton');
    expect(el.className).toContain('h-4');
  });

  it('renders the composed card skeleton', () => {
    const { container } = render(<CardSkeleton />);
    expect(container.querySelectorAll('.animate-skeleton').length).toBeGreaterThan(3);
  });

  it('renders the composed row skeleton', () => {
    const { container } = render(<RowSkeleton />);
    expect(container.querySelectorAll('.animate-skeleton').length).toBeGreaterThanOrEqual(3);
  });
});
