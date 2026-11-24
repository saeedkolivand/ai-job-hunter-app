/** AiNote — the structured fit beside the AI prose (#1446): chip, lists, stale label. */

import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { AiNote } from './AiNote';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@ajh/ui', () => ({
  Tag: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}));
vi.mock('lucide-react', () => ({ Sparkles: () => null }));

describe('AiNote', () => {
  it('renders the prose with no chip, lists or stale label when there is no fit data', () => {
    render(<AiNote notes="Lead with retention." fit={{}} />);
    expect(screen.getByText('Lead with retention.')).toBeTruthy();
    expect(screen.queryByTestId('fit-category')).toBeNull();
    expect(screen.queryByTestId('notes-stale')).toBeNull();
    expect(screen.queryByText(/autopilot\.fit\.gapsLabel/)).toBeNull();
  });

  it('renders the category chip, the gap list and the to-verify list from the structured fields', () => {
    render(
      <AiNote
        notes="Solid CRM background."
        fit={{
          fitCategory: 'explicitGap',
          mandatoryGaps: ['Italian C1 required, résumé shows B1'],
          toVerify: ['Italian C1', 'At least 7+ years of CRM experience'],
        }}
      />
    );
    expect(screen.getByTestId('fit-category').textContent).toBe(
      'autopilot.fit.category.explicitGap'
    );
    expect(screen.getByText(/Italian C1 required, résumé shows B1/)).toBeTruthy();
    expect(screen.getByText(/Italian C1; At least 7\+ years of CRM experience/)).toBeTruthy();
    expect(screen.getByText('autopilot.fit.gapsLabel:')).toBeTruthy();
    expect(screen.getByText('autopilot.fit.verifyLabel:')).toBeTruthy();
  });

  it('shows the "may be out of date" label only when the note is stale', () => {
    const { rerender } = render(<AiNote notes="n" fit={{ fitCategory: 'unknown' }} />);
    expect(screen.queryByTestId('notes-stale')).toBeNull();
    rerender(<AiNote notes="n" fit={{ fitCategory: 'unknown', notesStale: true }} />);
    expect(screen.getByTestId('notes-stale').textContent).toBe('autopilot.fit.stale');
  });
});
