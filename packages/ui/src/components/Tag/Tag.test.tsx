import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

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

  it('closable: names the close button per tag so siblings are distinguishable', () => {
    render(
      <Tag closable closeLabel="Remove filter: rust">
        rust
      </Tag>
    );
    // Several closable tags in a row would otherwise all announce as "Close".
    expect(screen.getByRole('button', { name: 'Remove filter: rust' })).toBeInTheDocument();
  });

  it('closable: pads the close button up to a ~24px target without shifting layout', () => {
    render(<Tag closable>x</Tag>);
    const close = screen.getByRole('button', { name: 'Close' });
    // 11px glyph + p-1.5 (6px each side) ≈ 23.5px — WCAG 2.5.8. `p-1` measured
    // only 19.5px. The negative margins cancel the padding so nothing moves.
    expect(close.className).toContain('p-1.5');
    expect(close.className).toContain('-m-1.5');
    expect(close.className).toContain('-mr-2');
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

describe('Tag — clickable label', () => {
  it('exposes the label as a real button: tabbable, Enter and Space both activate', async () => {
    const onClick = vi.fn();
    render(<Tag onClick={onClick}>recruiter</Tag>);
    await userEvent.tab();
    const label = screen.getByRole('button', { name: 'recruiter' });
    expect(label).toHaveFocus();
    await userEvent.keyboard('{Enter}');
    await userEvent.keyboard(' ');
    expect(onClick).toHaveBeenCalledTimes(2);
  });

  it('keeps the close button a SIBLING of the clickable label (buttons cannot nest)', () => {
    render(
      <Tag onClick={() => {}} closable>
        rust
      </Tag>
    );
    const label = screen.getByRole('button', { name: 'rust' });
    const close = screen.getByRole('button', { name: 'Close' });
    // Nested buttons are invalid markup and the inner one is unreachable.
    expect(label.contains(close)).toBe(false);
    expect(close.contains(label)).toBe(false);
  });

  it('emits no control at all when onClick is absent (a plain chip stays a span)', () => {
    render(<Tag>plain</Tag>);
    expect(screen.queryByRole('button')).toBeNull();
    expect(screen.getByText('plain').tagName).toBe('SPAN');
  });
});
