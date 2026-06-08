import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';

import { GlassCard } from './GlassCard';

describe('GlassCard', () => {
  it('renders children with the flat surface tone by default', () => {
    render(<GlassCard>content</GlassCard>);
    const card = screen.getByText('content');
    expect(card.className).toContain('surface-card');
    // The flat default is not glass, so it carries no frosted highlight.
    expect(card.className).not.toContain('glass-highlight');
  });

  it('opts into frosted glass with tone="glass"', () => {
    render(<GlassCard tone="glass">g</GlassCard>);
    const card = screen.getByText('g');
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
