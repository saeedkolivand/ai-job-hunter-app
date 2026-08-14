/**
 * ThinkingBubble — `defaultExpanded` (H1) and `done`-driven auto-collapse.
 *
 * `motion/react` is collapsed to plain wrappers so `AnimatePresence`'s exit
 * animation doesn't leave the collapsed content in the DOM mid-test.
 */
import type React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('motion/react', () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  motion: {
    div: ({ children, ...rest }: React.HTMLAttributes<HTMLDivElement>) => (
      <div {...rest}>{children}</div>
    ),
  },
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { ThinkingBubble } from './index';

describe('ThinkingBubble — defaultExpanded (H1)', () => {
  it('defaults open (defaultExpanded omitted) — the historical behavior', () => {
    render(<ThinkingBubble thinking="reasoning tokens" />);
    expect(screen.getByRole('button')).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('reasoning tokens')).toBeInTheDocument();
  });

  it('starts collapsed when defaultExpanded=false', () => {
    render(<ThinkingBubble thinking="reasoning tokens" defaultExpanded={false} />);
    expect(screen.getByRole('button')).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByText('reasoning tokens')).not.toBeInTheDocument();
  });

  it('expands on click from a collapsed default', async () => {
    const user = userEvent.setup();
    render(<ThinkingBubble thinking="reasoning tokens" defaultExpanded={false} />);
    await user.click(screen.getByRole('button'));
    expect(screen.getByRole('button')).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('reasoning tokens')).toBeInTheDocument();
  });

  it('renders nothing when there is no thinking text yet, regardless of defaultExpanded', () => {
    const { container } = render(<ThinkingBubble thinking="" defaultExpanded={false} />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe('ThinkingBubble — done state', () => {
  it('auto-collapses once done flips true with existing content', () => {
    const { rerender } = render(<ThinkingBubble thinking="reasoning tokens" done={false} />);
    expect(screen.getByRole('button')).toHaveAttribute('aria-expanded', 'true');

    rerender(<ThinkingBubble thinking="reasoning tokens" done />);
    expect(screen.getByRole('button')).toHaveAttribute('aria-expanded', 'false');
  });

  it('shows the "reasoning complete" label once done', () => {
    render(<ThinkingBubble thinking="reasoning tokens" done />);
    expect(screen.getByText('aiGenerate.reasoningComplete')).toBeInTheDocument();
  });
});
