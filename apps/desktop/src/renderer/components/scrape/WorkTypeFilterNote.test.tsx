/**
 * WorkTypeFilterNote — the work-type sibling of LocationFilterNote, both
 * generated from the same `FilterCapabilityNote` body in `LocationFilterNote.tsx`.
 * Covers the same contract with `active`/`supportsWorkType` in place of
 * `hasLocation`/`supportsLocation` — see `LocationFilterNote.test.tsx` for the
 * shared-body rationale.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

import { WorkTypeFilterNote } from './LocationFilterNote';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

function board(id: string, supportsWorkType?: boolean): BoardCatalogEntry {
  return {
    id,
    displayName: id,
    mode: 'api',
    auth: 'guest',
    listed: true,
    requiresCompany: false,
    supportsWorkType,
  };
}

describe('WorkTypeFilterNote', () => {
  it('renders nothing when no work type is selected', () => {
    render(<WorkTypeFilterNote boards={[board('greenhouse', false)]} active={false} />);
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('renders nothing when every selected board supports work type server-side', () => {
    render(<WorkTypeFilterNote boards={[board('smartrecruiters', true)]} active />);
    expect(screen.queryByRole('note')).toBeNull();
  });

  it('names ONLY the non-supporting boards when a work type is selected', () => {
    render(
      <WorkTypeFilterNote
        boards={[board('smartrecruiters', true), board('greenhouse', false), board('lever', false)]}
        active
      />
    );
    const note = screen.getByRole('note');
    expect(note.textContent).toContain('jobs.workType.filterHint');
    expect(note.textContent).toContain('jobs.boards.greenhouse');
    expect(note.textContent).toContain('jobs.boards.lever');
    expect(note.textContent).not.toContain('jobs.boards.smartrecruiters');
  });

  it('treats an absent supportsWorkType flag as "does not support work type"', () => {
    render(<WorkTypeFilterNote boards={[board('greenhouse', undefined)]} active />);
    expect(screen.getByRole('note').textContent).toContain('jobs.boards.greenhouse');
  });
});
