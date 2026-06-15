import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { Tag } from './Tag';

describe('Tag', () => {
  it('renders children with the neutral default style when no color is set', () => {
    render(<Tag>draft</Tag>);
    const el = screen.getByText('draft');
    expect(el.className).toContain('text-foreground/70');
  });

  it('maps a status color to its themed class (legible in light + dark)', () => {
    render(<Tag color="success">passed</Tag>);
    expect(screen.getByText('passed').className).toContain('text-emerald-400');
  });

  it('renders a custom CSS color as a solid inline fill', () => {
    render(<Tag color="#8b5cf6">custom</Tag>);
    const el = screen.getByText('custom');
    expect(el.style.backgroundColor).not.toBe('');
    expect(el.style.color).toBe('rgb(255, 255, 255)');
  });

  it('drops the border when bordered={false}', () => {
    render(<Tag bordered={false}>flat</Tag>);
    expect(screen.getByText('flat').className).toContain('border-transparent');
  });

  it('closable: fires onClose and unmounts on close click', () => {
    const onClose = vi.fn();
    render(
      <Tag closable onClose={onClose}>
        bye
      </Tag>
    );
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('bye')).toBeNull();
  });

  it('closable: stays mounted when onClose calls preventDefault', () => {
    render(
      <Tag closable onClose={(e) => e.preventDefault()}>
        keep
      </Tag>
    );
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.getByText('keep')).toBeInTheDocument();
  });
});

describe('Tag.CheckableTag', () => {
  it('reflects checked via aria-pressed and toggles on click', () => {
    const onChange = vi.fn();
    render(
      <Tag.CheckableTag checked={false} onChange={onChange}>
        recruiter
      </Tag.CheckableTag>
    );
    const btn = screen.getByRole('button', { name: 'recruiter' });
    expect(btn).toHaveAttribute('aria-pressed', 'false');
    fireEvent.click(btn);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('does not fire onChange when disabled', () => {
    const onChange = vi.fn();
    render(
      <Tag.CheckableTag checked={false} disabled onChange={onChange}>
        team
      </Tag.CheckableTag>
    );
    fireEvent.click(screen.getByRole('button', { name: 'team' }));
    expect(onChange).not.toHaveBeenCalled();
  });
});
