import { Tag } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { SectionLabel } from './SectionLabel';

describe('SectionLabel', () => {
  it('renders children', () => {
    render(<SectionLabel>Details</SectionLabel>);
    expect(screen.getByText('Details')).toBeInTheDocument();
  });

  it('renders an optional icon', () => {
    const { container } = render(<SectionLabel icon={Tag}>Labelled</SectionLabel>);
    expect(container.querySelector('svg')).toBeTruthy();
  });
});
