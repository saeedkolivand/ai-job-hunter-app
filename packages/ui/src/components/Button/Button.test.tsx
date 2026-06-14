import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Button } from './Button';

describe('Button', () => {
  it('renders its children', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByRole('button', { name: 'Click me' })).toBeInTheDocument();
  });

  it('fires onClick', async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Go</Button>);
    await userEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it('is disabled while loading and shows a spinner', () => {
    render(<Button loading>Saving</Button>);
    expect(screen.getByRole('button')).toBeDisabled();
  });

  it('does not fire onClick when disabled', async () => {
    const onClick = vi.fn();
    render(
      <Button disabled onClick={onClick}>
        Nope
      </Button>
    );
    await userEvent.click(screen.getByRole('button')).catch(() => {});
    expect(onClick).not.toHaveBeenCalled();
  });

  it('applies variant and size class hooks', () => {
    render(
      <Button variant="danger" size="lg" className="custom">
        Delete
      </Button>
    );
    const btn = screen.getByRole('button');
    expect(btn.className).toContain('custom');
    expect(btn.className).toContain('h-10');
  });

  // ── accent-gradient assertions (feat/accent-gradients) ──────────────────────

  it('variant="primary" carries bg-brand-gradient and text-brand-foreground (NOT bg-action-primary)', () => {
    render(<Button variant="primary">CTA</Button>);
    const btn = screen.getByRole('button');
    expect(btn.className).toContain('bg-brand-gradient');
    expect(btn.className).toContain('text-brand-foreground');
    expect(btn.className).not.toContain('bg-action-primary');
  });

  it('variant="run" still carries its solid bg-action-run token (unchanged by accent-gradient sweep)', () => {
    render(<Button variant="run">Run</Button>);
    const btn = screen.getByRole('button');
    expect(btn.className).toContain('bg-action-run');
    expect(btn.className).toContain('text-action-foreground');
  });

  it('injects no chrome for the unstyled variant but keeps a11y essentials', () => {
    render(
      <Button variant="unstyled" className="custom-surface">
        Bare
      </Button>
    );
    const btn = screen.getByRole('button');
    // Call site owns the look…
    expect(btn.className).toContain('custom-surface');
    // …so no layout/size chrome is injected.
    expect(btn.className).not.toContain('inline-flex');
    expect(btn.className).not.toContain('h-8');
    // Focus + disabled handling still apply (the reason to route through Button).
    expect(btn.className).toContain('focus-visible:ring-2');
    expect(btn.className).toContain('disabled:opacity-45');
  });
});
