import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { Input } from './Input';

describe('Input', () => {
  it('renders a text input by default and accepts typing', async () => {
    const onChange = vi.fn();
    render(<Input placeholder="Email" onChange={onChange} />);
    const input = screen.getByPlaceholderText('Email');
    expect(input).toHaveAttribute('type', 'text');
    await userEvent.type(input, 'hi');
    expect(onChange).toHaveBeenCalled();
  });

  it('honours an explicit type and custom class', () => {
    render(<Input type="password" className="extra" placeholder="pw" />);
    const input = screen.getByPlaceholderText('pw');
    expect(input).toHaveAttribute('type', 'password');
    expect(input.className).toContain('extra');
  });

  it('applies the default (non-glass) variant styles', () => {
    render(<Input variant="default" placeholder="x" />);
    expect(screen.getByPlaceholderText('x').className).toContain('bg-white/5');
  });

  it('injects no field chrome for the unstyled variant', () => {
    render(<Input variant="unstyled" className="bare" placeholder="u" />);
    const input = screen.getByPlaceholderText('u');
    expect(input.className).toContain('bare');
    expect(input.className).not.toContain('input-field');
    expect(input.className).not.toContain('glass-dropdown');
  });
});
