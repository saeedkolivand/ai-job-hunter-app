import { Sparkles } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { FloatingIcon } from './FloatingIcon';

describe('FloatingIcon', () => {
  it('renders the icon by default', () => {
    const { container } = render(<FloatingIcon icon={Sparkles} />);
    expect(container.querySelector('svg')).toBeTruthy();
  });

  it('renders children instead of the icon when provided', () => {
    render(<FloatingIcon icon={Sparkles}>👋</FloatingIcon>);
    expect(screen.getByText('👋')).toBeInTheDocument();
  });
});
