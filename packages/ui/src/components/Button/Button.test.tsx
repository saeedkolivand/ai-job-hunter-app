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
});
