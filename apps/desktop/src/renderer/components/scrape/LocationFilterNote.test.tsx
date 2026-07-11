/**
 * LocationFilterNote — honest picker hint (PR F, job-search trust program).
 *
 * Covers:
 *  - Gate on a location being set: absent without one, present with one.
 *  - Names ONLY the selected boards that don't filter by location server-side
 *    (`supportsLocation` falsy); location-supporting boards are never listed.
 *  - An absent flag reads as "does not support location" (contract semantics).
 *
 * @ajh/translations is a readable identity mock so the localized label + each
 * board key are assertable without a real i18n instance (parity is covered
 * separately in LocationFilterNote.i18n.test.ts).
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

import { LocationFilterNote } from './LocationFilterNote';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

function board(id: string, supportsLocation?: boolean): BoardCatalogEntry {
  return {
    id,
    displayName: id,
    mode: 'api',
    auth: 'guest',
    listed: true,
    requiresCompany: false,
    supportsLocation,
  };
}

describe('LocationFilterNote', () => {
  it('renders nothing when no location is set', () => {
    render(<LocationFilterNote boards={[board('greenhouse', false)]} hasLocation={false} />);
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('renders nothing when every selected board supports location server-side', () => {
    render(
      <LocationFilterNote
        boards={[board('aggregator', true), board('linkedin', true)]}
        hasLocation
      />
    );
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('renders nothing for an empty selection', () => {
    render(<LocationFilterNote boards={[]} hasLocation />);
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('names ONLY the non-supporting boards when a location is set', () => {
    render(
      <LocationFilterNote
        boards={[board('aggregator', true), board('greenhouse', false), board('lever', false)]}
        hasLocation
      />
    );
    const note = screen.getByRole('note');
    expect(note.textContent).toContain('jobs.locationFilterHint');
    expect(note.textContent).toContain('jobs.boards.greenhouse');
    expect(note.textContent).toContain('jobs.boards.lever');
    // A board that DOES filter by location server-side must never be named.
    expect(note.textContent).not.toContain('jobs.boards.aggregator');
  });

  it('treats an absent supportsLocation flag as "does not support location"', () => {
    render(<LocationFilterNote boards={[board('greenhouse', undefined)]} hasLocation />);
    expect(screen.getByRole('note').textContent).toContain('jobs.boards.greenhouse');
  });
});
