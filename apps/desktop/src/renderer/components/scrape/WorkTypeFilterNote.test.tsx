/**
 * WorkTypeFilterNote — the work-type sibling of LocationFilterNote, both
 * generated from the same `FilterCapabilityNote` body in `LocationFilterNote.tsx`.
 * Covers the same gating contract with `active`/`supportsWorkType` in place of
 * `hasLocation`/`supportsLocation` — see `LocationFilterNote.test.tsx` for the
 * shared-body rationale.
 *
 * Presentation differs deliberately: only `smartrecruiters` supports work type
 * upstream (1 of 26 boards, vs 4 of 26 for location), so a chip-per-board list
 * would name almost the entire selection — noise, not a hint. This variant
 * collapses to a `{{count}}`/`{{total}}` summary sentence instead of chips.
 */

import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type { BoardCatalogEntry } from '@ajh/shared';

import { WorkTypeFilterNote } from './LocationFilterNote';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: { count?: number; total?: number }) =>
      opts ? `${key}|count=${opts.count}|total=${opts.total}` : key,
  }),
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

  it('renders a count summary, never a chip-per-board list, when boards lack upstream support', () => {
    render(
      <WorkTypeFilterNote
        boards={[board('smartrecruiters', true), board('greenhouse', false), board('lever', false)]}
        active
      />
    );
    const note = screen.getByRole('note');
    // 2 of the 3 SELECTED boards don't support it upstream — counted against
    // the full selection, not just the affected subset.
    expect(note.textContent).toContain('jobs.workType.filterSummary|count=2|total=3');
    // No per-board chip — this is exactly the noise the summary replaces.
    expect(note.textContent).not.toContain('jobs.boards.greenhouse');
    expect(note.textContent).not.toContain('jobs.boards.lever');
    expect(note.textContent).not.toContain('jobs.boards.smartrecruiters');
  });

  it('treats an absent supportsWorkType flag as "does not support work type"', () => {
    render(<WorkTypeFilterNote boards={[board('greenhouse', undefined)]} active />);
    expect(screen.getByRole('note').textContent).toContain(
      'jobs.workType.filterSummary|count=1|total=1'
    );
  });
});
