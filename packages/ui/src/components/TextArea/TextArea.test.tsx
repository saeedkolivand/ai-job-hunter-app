import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TextArea } from './TextArea';

describe('TextArea', () => {
  it('renders and accepts input', async () => {
    const onChange = vi.fn();
    render(<TextArea placeholder="notes" onChange={onChange} />);
    await userEvent.type(screen.getByPlaceholderText('notes'), 'a');
    expect(onChange).toHaveBeenCalled();
  });

  it('applies the glass variant class', () => {
    render(<TextArea variant="glass" placeholder="g" />);
    expect(screen.getByPlaceholderText('g').className).toContain('glass-dropdown');
  });

  it('applies the default variant class', () => {
    render(<TextArea variant="default" placeholder="d" />);
    expect(screen.getByPlaceholderText('d').className).toContain('bg-transparent');
  });
});
