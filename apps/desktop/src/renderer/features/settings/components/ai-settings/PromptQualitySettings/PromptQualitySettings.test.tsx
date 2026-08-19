/**
 * PromptQualitySettings — the canonical Settings home for the global
 * `promptQuality` preference (also settable from the AI Generate wizard and
 * Analyze — all three read/write the SAME `usePreferencesStore` field, so this
 * only needs to prove the store write-through, not re-verify the sibling UIs).
 *
 * Scope honesty: `resolveEffectiveTier` (`lib/generate/provider-context.ts`)
 * returns `'large'` unconditionally for anything but `ollama`, ignoring this
 * preference entirely — the second describe block pins that the control says
 * so whenever the active provider isn't Ollama, and stays quiet when it is.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key} ${Object.values(params).join(' ')}` : key,
  }),
}));

const setPromptQuality = vi.fn();
const mockUsePromptQuality = vi.fn();

vi.mock('@/store/preferences-store', () => ({
  usePromptQuality: () => mockUsePromptQuality(),
  usePreferencesStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setPromptQuality }),
}));

import { PromptQualitySettings } from './index';

describe('PromptQualitySettings — reads/writes the shared store', () => {
  it('reflects the current promptQuality from usePromptQuality', () => {
    mockUsePromptQuality.mockReturnValue('compact');
    render(<PromptQualitySettings activeProvider="ollama" />);

    expect(screen.getByRole('radio', { name: /fast/i })).toHaveAttribute('aria-checked', 'true');
  });

  it('calls the SAME store setter StepFineTune/AnalyzeLeftPanel use, on selection', async () => {
    mockUsePromptQuality.mockReturnValue('auto');
    const user = userEvent.setup();
    render(<PromptQualitySettings activeProvider="ollama" />);

    await user.click(screen.getByRole('radio', { name: /full/i }));

    expect(setPromptQuality).toHaveBeenCalledWith('full');
  });
});

describe('PromptQualitySettings — scope honesty (Ollama-only)', () => {
  it('states the setting does not apply when the active provider is not Ollama', () => {
    mockUsePromptQuality.mockReturnValue('auto');
    render(<PromptQualitySettings activeProvider="openai" />);

    // Exact text, not a prefix regex: `/…ollamaOnlyNote/` also matched a
    // typo'd `ollamaOnlyNoteX` key AND a component that dropped the
    // `{ provider }` interpolation, so it survived both mutants. The trailing
    // ` OpenAI` comes from the `t` mock above appending the param values —
    // asserting it pins BOTH the key and the interpolation.
    expect(screen.getByText('settings.promptQuality.ollamaOnlyNote OpenAI')).toBeInTheDocument();
  });

  it('stays silent about scope when the active provider IS Ollama', () => {
    mockUsePromptQuality.mockReturnValue('auto');
    render(<PromptQualitySettings activeProvider="ollama" />);

    expect(screen.queryByText(/settings\.promptQuality\.ollamaOnlyNote/)).not.toBeInTheDocument();
  });
});
