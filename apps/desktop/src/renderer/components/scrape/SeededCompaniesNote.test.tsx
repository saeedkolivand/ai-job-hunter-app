/**
 * SeededCompaniesNote — company-scoped ATS board disclosure (#621).
 *
 * Covers:
 *  - Renders nothing when no selected board carries `seededCompanies`.
 *  - Renders nothing for an empty selection.
 *  - Names ONLY the seeded boards (a selected board without `seededCompanies`
 *    is never mentioned).
 *  - Truncates to the first 5 company names and signals the remainder via the
 *    pluralized "more" key (exact interpolated count covered in the real-i18n
 *    parity test — this identity-mock test only proves the truncation math).
 *  - Exactly 5 seeded companies (the boundary): all 5 shown, no "more" suffix.
 *  - Full company list is always available via the native `title` tooltip,
 *    including when nothing was truncated.
 *
 * @ajh/translations is a readable identity mock (mirrors LocationFilterNote.test.tsx)
 * so keys are assertable without a real i18n instance; parity + real pluralized
 * interpolation are covered in SeededCompaniesNote.i18n.test.ts.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

import { SeededCompaniesNote } from './SeededCompaniesNote';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

function board(id: string, seededCompanies?: string[]): BoardCatalogEntry {
  return {
    id,
    displayName: id,
    mode: 'api',
    auth: 'guest',
    listed: true,
    requiresCompany: true,
    seededCompanies,
  };
}

describe('SeededCompaniesNote', () => {
  it('renders nothing for an empty selection', () => {
    render(<SeededCompaniesNote boards={[]} />);
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('renders nothing when no selected board carries seededCompanies', () => {
    render(<SeededCompaniesNote boards={[board('linkedin'), board('greenhouse', [])]} />);
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('names ONLY the seeded boards, truncates to 5 names, and signals the remainder', () => {
    render(
      <SeededCompaniesNote
        boards={[
          board('linkedin'),
          board('greenhouse', ['Stripe', 'Airbnb', 'OpenAI', 'Bosch', 'N26', 'Lyft']),
        ]}
      />
    );
    const note = screen.getByRole('note');
    expect(note.textContent).toContain('jobs.boards.greenhouse');
    expect(note.textContent).not.toContain('jobs.boards.linkedin');
    expect(note.textContent).toContain('Stripe');
    expect(note.textContent).toContain('Airbnb');
    expect(note.textContent).toContain('OpenAI');
    expect(note.textContent).toContain('Bosch');
    expect(note.textContent).toContain('N26');
    // 6th name is truncated away from the inline text …
    expect(note.textContent).not.toContain('Lyft');
    // … but the "more" key fired (real count/pluralization verified separately).
    expect(note.textContent).toContain('autopilot.wizard.target.seededCompanies.more');
  });

  it('does not show the "more" key when the company count is within the shown limit', () => {
    render(<SeededCompaniesNote boards={[board('lever', ['Figma', 'Notion'])]} />);
    const note = screen.getByRole('note');
    expect(note.textContent).toContain('Figma');
    expect(note.textContent).toContain('Notion');
    expect(note.textContent).not.toContain('autopilot.wizard.target.seededCompanies.more');
  });

  it('shows all 5 names with no "more" suffix at the exactly-5 boundary', () => {
    const companies = ['Stripe', 'Airbnb', 'OpenAI', 'Bosch', 'N26'];
    render(<SeededCompaniesNote boards={[board('greenhouse', companies)]} />);
    const note = screen.getByRole('note');
    for (const name of companies) {
      expect(note.textContent).toContain(name);
    }
    expect(note.textContent).not.toContain('autopilot.wizard.target.seededCompanies.more');
  });

  it('exposes the full company list via the native title tooltip', () => {
    const companies = ['Stripe', 'Airbnb', 'OpenAI', 'Bosch', 'N26', 'Lyft'];
    render(<SeededCompaniesNote boards={[board('greenhouse', companies)]} />);
    const note = screen.getByRole('note');
    expect(note.querySelector(`[title="${companies.join(', ')}"]`)).not.toBeNull();
  });
});
