/**
 * RewritePopover — the two measured defects on the result: a result identical to
 * the selection is a neutral "unchanged" state and not an Accept-able success,
 * and a numeric limit in the instruction is verified by code with exactly ONE
 * re-ask.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/react';

import { rewriteSelection } from '@/lib/generate';

import { acceptButton, renderPopover, runInstruction, SELECTION } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@/lib/generate', async () => (await import('./test-mocks')).generateMock());
vi.mock('motion/react', async () => (await import('./test-mocks')).motionMock());
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./test-mocks')).uiMock(await importOriginal())
);

describe('RewritePopover — unchanged result (C2)', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('shows the neutral unchanged notice and DISABLES Accept when the result echoes the selection', async () => {
    // The measured no-op: the result differed from the input by one comma.
    vi.mocked(rewriteSelection).mockResolvedValue(`${SELECTION},`);
    renderPopover();

    await runInstruction('tighten this');

    expect(screen.getByText('aiGenerate.rewrite.unchanged')).toBeTruthy();
    // Neutral, NOT an error.
    expect(screen.queryByText('aiGenerate.rewrite.failed')).toBeNull();
    expect(acceptButton().disabled).toBe(true);
    // Regenerate stays live so the user can ask again.
    expect(screen.getByRole('button', { name: 'aiGenerate.rewrite.regenerate' })).toBeTruthy();
  });

  it('leaves Accept enabled and shows no notice for a genuinely different result', async () => {
    vi.mocked(rewriteSelection).mockResolvedValue('a completely different sentence');
    renderPopover();

    await runInstruction('tighten this');

    expect(screen.queryByText('aiGenerate.rewrite.unchanged')).toBeNull();
    expect(acceptButton().disabled).toBe(false);
  });
});

describe('RewritePopover — code-enforced length limit (C4)', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('re-asks exactly ONCE with the measured overshoot when the result breaks the parsed limit', async () => {
    const over = 'x'.repeat(30);
    const inside = 'y'.repeat(10);
    vi.mocked(rewriteSelection)
      .mockResolvedValueOnce(over)
      .mockResolvedValueOnce(inside)
      .mockResolvedValue('never reached');
    renderPopover();

    await runInstruction('rewrite this under 20 characters');

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(2);
    const secondInstruction = vi.mocked(rewriteSelection).mock.calls[1]?.[0].instruction as string;
    expect(secondInstruction).toContain('rewrite this under 20 characters');
    expect(secondInstruction).toContain('30 characters');
    expect(secondInstruction).toContain('cut at least 10 characters');
    // Inside the limit on the retry → no count line, Accept live.
    expect(screen.queryByText('aiGenerate.rewrite.overLimit.chars')).toBeNull();
    expect(acceptButton().disabled).toBe(false);
  });

  it('never asks a third time, and shows the count next to a still-enabled Accept', async () => {
    vi.mocked(rewriteSelection).mockResolvedValue('x'.repeat(30));
    renderPopover();

    await runInstruction('rewrite this under 20 characters');

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(2);
    expect(screen.getByText('aiGenerate.rewrite.overLimit.chars')).toBeTruthy();
    // Advisory, not a block: the user decides.
    expect(acceptButton().disabled).toBe(false);
  });

  it('does not re-ask at all when the instruction carries no numeric limit', async () => {
    vi.mocked(rewriteSelection).mockResolvedValue('x'.repeat(400));
    renderPopover();

    await runInstruction('make this punchier');

    expect(vi.mocked(rewriteSelection)).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('aiGenerate.rewrite.overLimit.chars')).toBeNull();
  });
});
