// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import type { AgentRole } from '@/data/agent-fleet';

import { NodeButton } from './NodeButton';

afterEach(cleanup);

describe('NodeButton', () => {
  it.each<AgentRole>(['author', 'critic', 'cross'])(
    'reflects the `selected` prop on aria-expanded for role=%s',
    (roleClass) => {
      const { rerender } = render(
        <NodeButton
          name="rust-backend-author"
          roleClass={roleClass}
          selected={false}
          onSelect={vi.fn()}
          onHover={vi.fn()}
        />
      );
      const btn = screen.getByRole('button', { name: 'rust-backend-author' });
      expect(btn.getAttribute('aria-expanded')).toBe('false');

      rerender(
        <NodeButton
          name="rust-backend-author"
          roleClass={roleClass}
          selected={true}
          onSelect={vi.fn()}
          onHover={vi.fn()}
        />
      );
      expect(btn.getAttribute('aria-expanded')).toBe('true');
    }
  );

  it('adds node-author only for roleClass="author"', () => {
    render(
      <NodeButton
        name="x"
        roleClass="author"
        selected={false}
        onSelect={vi.fn()}
        onHover={vi.fn()}
      />
    );
    expect(screen.getByRole('button').className).toContain('node-author');
    cleanup();

    render(
      <NodeButton
        name="x"
        roleClass="critic"
        selected={false}
        onSelect={vi.fn()}
        onHover={vi.fn()}
      />
    );
    expect(screen.getByRole('button').className).not.toContain('node-author');
  });

  it('calls onSelect(name) on click', () => {
    const onSelect = vi.fn();
    render(
      <NodeButton
        name="job-match-author"
        roleClass="author"
        selected={false}
        onSelect={onSelect}
        onHover={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'job-match-author' }));
    expect(onSelect).toHaveBeenCalledExactlyOnceWith('job-match-author');
  });

  it.each([
    ['mouseEnter', 'mouseLeave', 'job-match-author', null],
    ['focus', 'blur', 'job-match-author', null],
  ] as const)('%s sets the lit name, %s clears it', (enterEvent, leaveEvent, name, cleared) => {
    const onHover = vi.fn();
    render(
      <NodeButton
        name={name}
        roleClass="author"
        selected={false}
        onSelect={vi.fn()}
        onHover={onHover}
      />
    );
    const btn = screen.getByRole('button', { name });

    fireEvent[enterEvent](btn);
    expect(onHover).toHaveBeenLastCalledWith(name);

    fireEvent[leaveEvent](btn);
    expect(onHover).toHaveBeenLastCalledWith(cleared);
  });
});
