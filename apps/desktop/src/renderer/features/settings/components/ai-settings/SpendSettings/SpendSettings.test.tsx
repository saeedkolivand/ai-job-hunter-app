/**
 * SpendSettings — today total, per-provider breakdown, and the loading/empty/
 * error states, all driven by a mocked `useSpendSummary`.
 *
 * i18n is stubbed to return the key verbatim (matches the EmbeddingsSettings
 * test pattern), so assertions match on the localization key rather than the
 * rendered English/German copy.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const mockUseSpendSummary = vi.fn();

vi.mock('@/services', () => ({
  useSpendSummary: () => mockUseSpendSummary(),
}));

import { SpendSettings } from './index';

describe('SpendSettings — loaded with data', () => {
  it('renders the today total and a per-provider row', () => {
    mockUseSpendSummary.mockReturnValue({
      data: {
        today: { inputTokens: 12431, outputTokens: 3204, estCostUsd: 0.42 },
        perProvider: [
          { provider: 'openai', inputTokens: 12431, outputTokens: 3204, estCostUsd: 0.31 },
        ],
      },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('~$0.42')).toBeInTheDocument();
    expect(screen.getByText('~$0.31')).toBeInTheDocument();
    expect(screen.getByText('OpenAI')).toBeInTheDocument();
  });

  it('shows the estimated-cost disclaimer', () => {
    mockUseSpendSummary.mockReturnValue({
      data: {
        today: { inputTokens: 1, outputTokens: 1, estCostUsd: 0.01 },
        perProvider: [{ provider: 'openai', inputTokens: 1, outputTokens: 1, estCostUsd: 0.01 }],
      },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('settings.spend.disclaimer')).toBeInTheDocument();
  });

  it('renders "<$0.01" for a sub-cent estimate (never "~$0.00" or "$0")', () => {
    mockUseSpendSummary.mockReturnValue({
      data: {
        today: { inputTokens: 40, outputTokens: 10, estCostUsd: 0.005 },
        perProvider: [{ provider: 'openai', inputTokens: 40, outputTokens: 10, estCostUsd: 0.005 }],
      },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    const subCentAmounts = screen.getAllByText('<$0.01');
    expect(subCentAmounts).toHaveLength(2); // today total + the one provider row
    expect(screen.queryByText('~$0.00')).not.toBeInTheDocument();
    expect(screen.queryByText('$0')).not.toBeInTheDocument();
  });

  it('shows "local — free" for a zero-cost (local/CLI) provider row', () => {
    mockUseSpendSummary.mockReturnValue({
      data: {
        today: { inputTokens: 500, outputTokens: 100, estCostUsd: 0 },
        perProvider: [{ provider: 'ollama', inputTokens: 500, outputTokens: 100, estCostUsd: 0 }],
      },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('settings.spend.freeLocal')).toBeInTheDocument();
    expect(screen.queryByText('~$0.00')).not.toBeInTheDocument();
  });

  it('does not render a zero row with a `reason` as "local — free" (#1161)', () => {
    // A provider with real (paid) history but no activity in this window is a
    // zero row carrying `reason`, not a local/free provider — it must be
    // dropped from the list entirely rather than mislabeled.
    mockUseSpendSummary.mockReturnValue({
      data: {
        today: { inputTokens: 500, outputTokens: 100, estCostUsd: 0.31 },
        perProvider: [
          { provider: 'openai', inputTokens: 500, outputTokens: 100, estCostUsd: 0.31 },
          {
            provider: 'anthropic',
            inputTokens: 0,
            outputTokens: 0,
            estCostUsd: 0,
            reason: 'no spend in window',
          },
        ],
      },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('OpenAI')).toBeInTheDocument();
    expect(screen.queryByText('Anthropic')).not.toBeInTheDocument();
    expect(screen.queryByText('settings.spend.freeLocal')).not.toBeInTheDocument();
  });
});

describe('SpendSettings — empty', () => {
  it('shows EmptyState when there is no spend today', () => {
    mockUseSpendSummary.mockReturnValue({
      data: { today: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 }, perProvider: [] },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('settings.spend.emptyTitle')).toBeInTheDocument();
  });

  it('shows EmptyState when `perProvider` only has reason rows (#1161)', () => {
    // Every entry is a zero row with a reason (no real activity anywhere) —
    // the empty state must still fire, not an empty-looking list.
    mockUseSpendSummary.mockReturnValue({
      data: {
        today: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 },
        perProvider: [
          {
            provider: 'openai',
            inputTokens: 0,
            outputTokens: 0,
            estCostUsd: 0,
            reason: 'no spend in window',
          },
        ],
      },
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('settings.spend.emptyTitle')).toBeInTheDocument();
  });
});

describe('SpendSettings — loading', () => {
  it('shows row skeletons instead of data', () => {
    mockUseSpendSummary.mockReturnValue({
      data: undefined,
      isLoading: true,
      isError: false,
      refetch: vi.fn(),
    });

    const { container } = render(<SpendSettings />);

    expect(container.querySelectorAll('.animate-skeleton').length).toBeGreaterThan(0);
    expect(screen.queryByText('settings.spend.emptyTitle')).not.toBeInTheDocument();
  });
});

describe('SpendSettings — error', () => {
  it('shows ErrorState and never a blank panel', () => {
    mockUseSpendSummary.mockReturnValue({
      data: undefined,
      isLoading: false,
      isError: true,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('settings.spend.errorTitle')).toBeInTheDocument();
  });
});
