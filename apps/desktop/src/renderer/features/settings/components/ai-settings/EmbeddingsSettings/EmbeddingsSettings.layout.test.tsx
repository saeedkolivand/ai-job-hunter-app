/**
 * EmbeddingsSettings — toggle-row layout.
 *
 * Both symptoms reported against the shipped UI had ONE cause: the row is
 * `justify-between` with no gap, and the `<input>` had no `shrink-0`. With the
 * short semantic-scoring description there is slack and it looks right; with the
 * longer auto-index description the text block fills the row, so the copy butts
 * against the checkbox AND flexbox compresses the input below its `h-4 w-4` —
 * making it look like a different control rather than a squashed one.
 *
 * Asserted on the classes because that is where the bug lives: jsdom reports
 * zero geometry (see the repo's jsdom notes), so a computed-width assertion
 * would pass against the broken markup.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import type * as AjhUi from '@ajh/ui';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@/services', () => ({
  useEmbeddingStatus: () => ({ data: undefined, refetch: vi.fn() }),
  useSetEmbeddingConfig: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useReembedAll: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useJobEvents: (_handler: unknown) => undefined,
}));

vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return {
    ...actual,
    useNotification: () => ({
      open: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
      warning: vi.fn(),
      info: vi.fn(),
    }),
  };
});

import { EmbeddingsSettings } from './index';

describe('EmbeddingsSettings — toggle rows', () => {
  it('gives every toggle checkbox shrink-0 so a long description cannot squash it', () => {
    render(<EmbeddingsSettings />);

    const boxes = screen
      .getAllByRole('checkbox')
      .filter((el) => el.className.includes('accent-[var(--color-brand)]'));
    expect(boxes.length).toBeGreaterThanOrEqual(2);

    for (const box of boxes) {
      expect(box.className).toContain('shrink-0');
      expect(box.className).toContain('h-4');
      expect(box.className).toContain('w-4');
    }
  });

  it('keeps a gap between the description and the checkbox on every toggle row', () => {
    render(<EmbeddingsSettings />);

    const rows = screen
      .getAllByRole('checkbox')
      .filter((el) => el.className.includes('accent-[var(--color-brand)]'))
      .map((el) => el.parentElement);

    for (const row of rows) {
      expect(row).not.toBeNull();
      // `justify-between` alone collapses to zero once the text fills the row.
      expect(row?.className).toContain('gap-');
    }
  });

  it('labels both toggles for assistive tech', () => {
    render(<EmbeddingsSettings />);
    const boxes = screen
      .getAllByRole('checkbox')
      .filter((el) => el.className.includes('accent-[var(--color-brand)]'));
    for (const box of boxes) {
      expect(box.getAttribute('aria-label')).toBeTruthy();
    }
  });
});
