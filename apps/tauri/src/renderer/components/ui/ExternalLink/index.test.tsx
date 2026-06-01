import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { ExternalLink } from './index';

// ---------------------------------------------------------------------------
// Mock @/services so the component can render without a full provider tree.
// useOpenExternal returns a mutation-like object; we only need `mutate`.
// ---------------------------------------------------------------------------
const mockMutate = vi.fn();

vi.mock('@/services', () => ({
  useOpenExternal: () => ({ mutate: mockMutate }),
}));

describe('ExternalLink', () => {
  beforeEach(() => {
    mockMutate.mockClear();
  });

  it('renders an anchor with the correct href and children', () => {
    render(<ExternalLink href="https://example.com">Visit site</ExternalLink>);

    const link = screen.getByRole('link', { name: 'Visit site' });
    expect(link).toBeInTheDocument();
    expect(link).toHaveAttribute('href', 'https://example.com');
    expect(link).toHaveAttribute('rel', 'noreferrer');
  });

  it('prevents default navigation and calls openExternal.mutate with the href on click', () => {
    render(<ExternalLink href="https://example.com">Visit site</ExternalLink>);

    const link = screen.getByRole('link', { name: 'Visit site' });

    // fireEvent.click returns false when defaultPrevented is true.
    const notPrevented = fireEvent.click(link);
    expect(notPrevented).toBe(false);

    expect(mockMutate).toHaveBeenCalledTimes(1);
    expect(mockMutate).toHaveBeenCalledWith('https://example.com');
  });

  it('passes through extra props (className, title) onto the rendered anchor', () => {
    render(
      <ExternalLink href="https://example.com" className="my-link" title="Open externally">
        Visit site
      </ExternalLink>
    );

    const link = screen.getByRole('link', { name: 'Visit site' });
    expect(link).toHaveAttribute('class', 'my-link');
    expect(link).toHaveAttribute('title', 'Open externally');
  });
});
