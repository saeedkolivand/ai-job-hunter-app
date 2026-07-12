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
