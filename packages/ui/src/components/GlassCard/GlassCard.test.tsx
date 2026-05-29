import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { GlassCard } from './GlassCard';

describe('GlassCard', () => {
  it('renders children with the neutral tone by default', () => {
    render(<GlassCard>content</GlassCard>);
    const card = screen.getByText('content');
    expect(card.className).toContain('glass-card');
    expect(card.className).toContain('glass-highlight');
  });

  it('applies tone, glow and disables highlight', () => {
    render(
      <GlassCard tone="violet" glow highlight={false} className="x">
        c
      </GlassCard>
    );
    const card = screen.getByText('c');
    expect(card.className).toContain('glass-violet');
    expect(card.className).toContain('ring-brand/20');
    expect(card.className).not.toContain('glass-highlight');
    expect(card.className).toContain('x');
  });
});
