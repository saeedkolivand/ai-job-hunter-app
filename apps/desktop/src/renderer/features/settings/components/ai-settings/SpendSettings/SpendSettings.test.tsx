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

import type { AiSpendSummary } from '@ajh/shared';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

/** A query-result shape `useSpendSummary`'s callers actually read (`data`/
 *  `isLoading`/`isError`/`refetch`) — typed against the real contract so a
 *  fixture missing a required `AiSpendSummary` field (#1159 T3: `window`,
 *  `windowTotals`, `thinkingByModel`, `thinkingByModelWindow` all went
 *  non-optional in the same PR that added them) fails `tsc`, not silently
 *  renders a payload the backend never actually sends. */
interface SpendSummaryQueryResult {
  data: AiSpendSummary | undefined;
  isLoading: boolean;
  isError: boolean;
  refetch: () => void;
}

const mockUseSpendSummary = vi.fn<() => SpendSummaryQueryResult>();

vi.mock('@/services', () => ({
  useSpendSummary: () => mockUseSpendSummary(),
}));

import { SpendSettings } from './index';

/** One complete `AiSpendSummary`, so every test only has to spell out the
 *  fields it cares about (#1159 T3) — spreading this base means a field
 *  added to the contract tomorrow lands here with a real value instead of
 *  silently being absent from every fixture in this file. */
function spendSummary(overrides: Partial<AiSpendSummary> = {}): AiSpendSummary {
  return {
    window: { days: 1, from: 0, to: 0 },
    today: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 },
    windowTotals: { inputTokens: 0, outputTokens: 0, estCostUsd: 0 },
    perProvider: [],
    thinkingByModel: [],
    thinkingByModelWindow: 'allTime',
    ...overrides,
  };
}

describe('SpendSettings — loaded with data', () => {
  it('renders the today total and a per-provider row', () => {
    mockUseSpendSummary.mockReturnValue({
      data: spendSummary({
        today: { inputTokens: 12431, outputTokens: 3204, estCostUsd: 0.42 },
        perProvider: [
          { provider: 'openai', inputTokens: 12431, outputTokens: 3204, estCostUsd: 0.31 },
        ],
      }),
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
      data: spendSummary({
        today: { inputTokens: 1, outputTokens: 1, estCostUsd: 0.01 },
        perProvider: [{ provider: 'openai', inputTokens: 1, outputTokens: 1, estCostUsd: 0.01 }],
      }),
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('settings.spend.disclaimer')).toBeInTheDocument();
  });

  it('renders "<$0.01" for a sub-cent estimate (never "~$0.00" or "$0")', () => {
    mockUseSpendSummary.mockReturnValue({
      data: spendSummary({
        today: { inputTokens: 40, outputTokens: 10, estCostUsd: 0.005 },
        perProvider: [{ provider: 'openai', inputTokens: 40, outputTokens: 10, estCostUsd: 0.005 }],
      }),
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
      data: spendSummary({
        today: { inputTokens: 500, outputTokens: 100, estCostUsd: 0 },
        perProvider: [{ provider: 'ollama', inputTokens: 500, outputTokens: 100, estCostUsd: 0 }],
      }),
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
      data: spendSummary({
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
      }),
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    expect(screen.getByText('OpenAI')).toBeInTheDocument();
    expect(screen.queryByText('Anthropic')).not.toBeInTheDocument();
    expect(screen.queryByText('settings.spend.freeLocal')).not.toBeInTheDocument();
  });

  it('reads windowTotals, not today, for a multi-day window (#1161 days split)', () => {
    // #1159 T3: the mock never carried `windowTotals` before, so a call site
    // that collapsed `windowTotals` back onto `today` would have passed this
    // suite unnoticed — this pins the two as genuinely distinct on the
    // fixture the renderer actually receives.
    mockUseSpendSummary.mockReturnValue({
      data: spendSummary({
        window: { days: 7, from: 1, to: 2 },
        today: { inputTokens: 70, outputTokens: 30, estCostUsd: 0.5 },
        windowTotals: { inputTokens: 570, outputTokens: 230, estCostUsd: 3.5 },
        perProvider: [{ provider: 'openai', inputTokens: 70, outputTokens: 30, estCostUsd: 0.5 }],
      }),
      isLoading: false,
      isError: false,
      refetch: vi.fn(),
    });

    render(<SpendSettings />);

    // SpendSettings renders `today`, not `windowTotals` — the fixture's
    // distinct values catch a wiring bug that reads the wrong field. (Both
    // the today total and the one provider row happen to read "~$0.50"
    // here, hence `getAllByText`.)
    expect(screen.getAllByText('~$0.50')).toHaveLength(2);
    expect(screen.queryByText('~$3.50')).not.toBeInTheDocument();
  });
});

describe('SpendSettings — empty', () => {
  it('shows EmptyState when there is no spend today', () => {
    mockUseSpendSummary.mockReturnValue({
      data: spendSummary(),
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
      data: spendSummary({
        perProvider: [
          {
            provider: 'openai',
            inputTokens: 0,
            outputTokens: 0,
            estCostUsd: 0,
            reason: 'no spend in window',
          },
        ],
      }),
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
