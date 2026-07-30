// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';

import { ROUTES } from '@/data/agent-fleet';

import { IntakeSection } from './IntakeSection';

afterEach(cleanup);

// Mirrors IntakeSection.tsx's private ROUTE_ROW_CLASS map, for assertion only
// (not exported — it's an internal styling lookup, not part of the public API).
const ROUTE_ROW_CLASS: Record<'author' | 'critic' | 'secondary' | 'gate', string> = {
  author: '',
  critic: 'crit',
  secondary: 'sec',
  gate: 'gate',
};

describe('IntakeSection', () => {
  it('shows the placeholder and no aria-pressed issue before any selection', () => {
    render(<IntakeSection />);
    expect(screen.getByText('pick an issue to see who handles it.')).toBeTruthy();
    for (const btn of screen.getAllByRole('button')) {
      expect(btn.getAttribute('aria-pressed')).toBe('false');
    }
  });

  it.each(ROUTES)('clicking "$issue" displays its route: title, area, and every row', (route) => {
    const { container } = render(<IntakeSection />);
    fireEvent.click(screen.getByRole('button', { name: route.issue }));

    expect(screen.getByRole('button', { name: route.issue }).getAttribute('aria-pressed')).toBe(
      'true'
    );
    expect(screen.getByRole('heading', { name: route.title })).toBeTruthy();
    expect(screen.getByText(route.area)).toBeTruthy();

    const rows = container.querySelectorAll('.flow > li');
    expect(rows).toHaveLength(route.rows.length);
    route.rows.forEach((row, i) => {
      const li = rows[i];
      if (!li) throw new Error(`expected a row at index ${i}`);
      if (row.kind === 'area') {
        expect(li.querySelector('.lbl')?.textContent).toBe('area');
        expect(li.querySelector('.why')?.textContent).toBe(row.detail);
      } else {
        expect(li.querySelector('.lbl')?.textContent).toBe(row.kind);
        const nameEl = li.querySelector('.who b');
        expect(nameEl?.textContent).toBe(row.name);
        expect(nameEl?.className).toBe(ROUTE_ROW_CLASS[row.kind]);
        expect(li.querySelector('.why')?.textContent).toBe(`— ${row.why}`);
      }
    });
  });

  it('switching the selection updates aria-pressed on both the old and new issue buttons', () => {
    const first = ROUTES[0];
    const second = ROUTES[1];
    if (!first || !second) throw new Error('fixture needs at least two ROUTES');

    render(<IntakeSection />);
    fireEvent.click(screen.getByRole('button', { name: first.issue }));
    expect(screen.getByRole('button', { name: first.issue }).getAttribute('aria-pressed')).toBe(
      'true'
    );

    fireEvent.click(screen.getByRole('button', { name: second.issue }));
    expect(screen.getByRole('button', { name: first.issue }).getAttribute('aria-pressed')).toBe(
      'false'
    );
    expect(screen.getByRole('button', { name: second.issue }).getAttribute('aria-pressed')).toBe(
      'true'
    );
  });
});
