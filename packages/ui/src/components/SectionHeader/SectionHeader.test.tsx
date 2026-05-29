import { Settings } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { SectionHeader } from './SectionHeader';

describe('SectionHeader', () => {
  it('renders title and description', () => {
    render(<SectionHeader icon={Settings} title="General" description="App settings" />);
    expect(screen.getByText('General')).toBeInTheDocument();
    expect(screen.getByText('App settings')).toBeInTheDocument();
  });

  it('supports the small size variant', () => {
    render(<SectionHeader icon={Settings} title="Compact" size="sm" />);
    expect(screen.getByText('Compact').className).toContain('text-sm');
  });
});
