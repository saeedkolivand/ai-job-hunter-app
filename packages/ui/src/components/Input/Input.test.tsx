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
    expect(screen.getByPlaceholderText('x').className).toContain('bg-field');
  });

  it('injects no field chrome for the unstyled variant', () => {
    render(<Input variant="unstyled" className="bare" placeholder="u" />);
    const input = screen.getByPlaceholderText('u');
    expect(input.className).toContain('bare');
    expect(input.className).not.toContain('input-field');
    expect(input.className).not.toContain('glass-dropdown');
  });

  // Helper text must be ANNOUNCED with the field, not float as loose prose.
  describe('hint', () => {
    it('renders the hint and wires it via aria-describedby', () => {
      render(<Input placeholder="e" hint="Shared with the other tab" />);
      const input = screen.getByPlaceholderText('e');
      const id = input.getAttribute('aria-describedby');

      expect(id).toBeTruthy();
      expect(document.getElementById(id as string)?.textContent).toBe('Shared with the other tab');
    });

    it('adds no describedby when there is no hint', () => {
      render(<Input placeholder="e" />);
      expect(screen.getByPlaceholderText('e')).not.toHaveAttribute('aria-describedby');
    });

    it('appends to a caller-supplied aria-describedby rather than replacing it', () => {
      render(
        <>
          <span id="ext">external</span>
          <Input placeholder="e" aria-describedby="ext" hint="mine" />
        </>
      );
      const ids = screen.getByPlaceholderText('e').getAttribute('aria-describedby')?.split(' ');
      expect(ids).toContain('ext');
      expect(ids).toHaveLength(2);
    });

    it('works in the prefix/suffix wrapped mode too', () => {
      render(<Input placeholder="e" prefix={<span>@</span>} hint="wrapped hint" />);
      const id = screen.getByPlaceholderText('e').getAttribute('aria-describedby');
      expect(document.getElementById(id as string)?.textContent).toBe('wrapped hint');
    });
  });
});
