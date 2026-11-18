/** GenerationOutput — the "what an ATS sees" panel after a résumé export. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import { renderOutput, resetHarness } from './harness';

vi.mock('@/components/generation/AtsViewPanel', () => ({
  AtsViewPanel: ({ path }: { path: string | null }) =>
    path ? <div data-testid="ats-panel">{path}</div> : null,
}));

beforeEach(resetHarness);

describe('GenerationOutput — ATS view panel', () => {
  it('shows on the résumé tab for an exported path', () => {
    renderOutput({ activeOut: 'resume', atsViewPath: '/out/cv.pdf' });
    expect(screen.getByTestId('ats-panel')).toHaveTextContent('/out/cv.pdf');
  });

  it('is hidden without a path', () => {
    renderOutput({ activeOut: 'resume' });
    expect(screen.queryByTestId('ats-panel')).toBeNull();
  });

  it('is hidden on the cover letter tab', () => {
    renderOutput({ target: 'both', activeOut: 'cover', atsViewPath: '/out/cv.pdf' });
    expect(screen.queryByTestId('ats-panel')).toBeNull();
  });
});
