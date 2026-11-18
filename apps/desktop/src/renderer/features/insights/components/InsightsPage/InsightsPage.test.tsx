import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';

import type {
  InsightsOutcomeCounts,
  InsightsOutcomeGroup,
  InsightsOutcomesReport,
} from '@ajh/shared';

import { InsightsPage } from './index';

interface HookState {
  data?: InsightsOutcomesReport;
  isLoading?: boolean;
  isError?: boolean;
}
let mockState: HookState = {};
const refetch = vi.fn();

vi.mock('@/services', () => ({
  useInsightsOutcomes: () => ({ ...mockState, refetch }),
}));

const counts = (c: Partial<InsightsOutcomeCounts>): InsightsOutcomeCounts => ({
  positiveReply: 0,
  rejected: 0,
  noReply: 0,
  pending: 0,
  ...c,
});

const group = (
  dimension: InsightsOutcomeGroup['dimension'],
  key: string,
  c: InsightsOutcomeCounts,
  positiveRate: number | null
): InsightsOutcomeGroup => ({
  dimension,
  key,
  counts: c,
  rates:
    positiveRate === null
      ? null
      : { positiveReply: positiveRate, rejected: 0, noReply: 1 - positiveRate },
});

const report = (over: Partial<InsightsOutcomesReport> = {}): InsightsOutcomesReport => ({
  groups: [
    group(
      'board',
      'adzuna',
      counts({ positiveReply: 2, rejected: 2, noReply: 4, pending: 1 }),
      0.25
    ),
    group('board', 'remoteok', counts({ positiveReply: 1, noReply: 1 }), null),
    group('tailored', 'tailored', counts({ positiveReply: 3, noReply: 3 }), 0.5),
    group(
      'month',
      '2026-10',
      counts({ positiveReply: 3, rejected: 2, noReply: 5, pending: 1 }),
      0.3
    ),
  ],
  nudge: null,
  generatedAt: 0,
  ...over,
});

/** The listitem row of `label` inside `section`; throws (so the test fails loudly) when absent. */
const rowOf = (section: HTMLElement, label: string): HTMLElement => {
  const row = within(section)
    .getAllByRole('listitem')
    .find((li) => within(li).queryByText(label) !== null);
  if (!row) throw new Error(`no row labelled ${label}`);
  return row;
};

describe('InsightsPage', () => {
  beforeEach(() => {
    mockState = {};
    refetch.mockClear();
  });

  it('shows skeletons while loading', () => {
    mockState = { isLoading: true };
    const { container } = render(<InsightsPage />);
    expect(container.querySelector('.animate-pulse, [class*="skeleton"]')).not.toBeNull();
    expect(screen.queryByText('No application outcomes to show')).not.toBeInTheDocument();
  });

  it('shows a retryable error state', () => {
    mockState = { isError: true };
    render(<InsightsPage />);
    expect(screen.getByText('Insights could not be loaded')).toBeInTheDocument();
    screen.getByRole('button', { name: /retry|try again/i }).click();
    expect(refetch).toHaveBeenCalled();
  });

  it('shows the empty state when no application has been sent', () => {
    mockState = { data: report({ groups: [] }) };
    render(<InsightsPage />);
    expect(screen.getByText('No application outcomes to show')).toBeInTheDocument();
  });

  it('shows the headline totals and the positive-reply definition', () => {
    mockState = { data: report() };
    render(<InsightsPage />);
    // month totals: 3 positive of 10 decided, 1 pending
    expect(screen.getByText('30% positive reply rate')).toBeInTheDocument();
    expect(
      screen.getByText('3 of 10 decided applications got a positive reply. 1 still pending.')
    ).toBeInTheDocument();
    expect(screen.getByText(/reached screening or later/)).toBeInTheDocument();
  });

  it('shows no headline percentage below the minimum sample', () => {
    mockState = {
      data: report({
        groups: [group('month', '2026-10', counts({ positiveReply: 1, noReply: 2 }), null)],
      }),
    };
    render(<InsightsPage />);
    expect(screen.getByText('Not enough decided applications for a rate yet')).toBeInTheDocument();
    expect(screen.queryByText(/% positive reply rate/)).not.toBeInTheDocument();
  });

  it('renders a bar and counts for a rated group, counts only for a rate-less one', () => {
    mockState = { data: report() };
    render(<InsightsPage />);
    const boards = screen.getByRole('region', { name: 'By job board' });

    const adzuna = rowOf(boards, 'adzuna');
    expect(within(adzuna).getByRole('meter')).toHaveAttribute('aria-valuenow', '25');
    expect(within(adzuna).getByText('25% positive replies')).toBeInTheDocument();
    expect(
      within(adzuna).getByText('2 positive · 2 rejected · 4 no reply · 1 pending')
    ).toBeInTheDocument();
    expect(within(adzuna).queryByText('Not enough data yet')).not.toBeInTheDocument();

    const remoteok = rowOf(boards, 'remoteok');
    expect(within(remoteok).queryByRole('meter')).not.toBeInTheDocument();
    expect(within(remoteok).getByText('Not enough data yet')).toBeInTheDocument();
    expect(
      within(remoteok).getByText('1 positive · 0 rejected · 1 no reply · 0 pending')
    ).toBeInTheDocument();
    expect(within(remoteok).queryByText(/% positive replies/)).not.toBeInTheDocument();
  });

  it('labels tailored groups and skips dimensions with no groups', () => {
    mockState = { data: report() };
    render(<InsightsPage />);
    expect(screen.getByText('Tailored')).toBeInTheDocument();
    expect(screen.queryByText('By language')).not.toBeInTheDocument();
  });

  it('lists "Language not recorded" last in the language section, with its hint', () => {
    mockState = {
      data: report({
        groups: [
          group('targetLanguage', 'unknown', counts({ noReply: 20 }), 0),
          group('targetLanguage', 'en', counts({ positiveReply: 2, noReply: 3 }), 0.4),
        ],
      }),
    };
    render(<InsightsPage />);
    const section = screen.getByRole('region', { name: 'By language' });
    const rows = within(section).getAllByRole('listitem');
    expect(rows).toHaveLength(2);
    const [english, noGen] = [rowOf(section, 'English'), rowOf(section, 'Language not recorded')];
    expect(rows).toEqual([english, noGen]);
    const hint = within(noGen).getByText(
      /no generation, or whose generation has no target language/
    );
    const stats = within(noGen).getByText(/0 positive/);
    // label, bar, stats, then the hint: same rhythm as every other row
    expect(stats.compareDocumentPosition(hint) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('labels an unknown template "Template not recorded", not "Language not recorded"', () => {
    mockState = {
      data: report({
        groups: [group('template', 'unknown', counts({ positiveReply: 3, noReply: 4 }), 0.43)],
      }),
    };
    render(<InsightsPage />);
    const section = screen.getByRole('region', { name: 'By résumé template' });
    expect(within(section).getByText('Template not recorded')).toBeInTheDocument();
    expect(
      within(section).getByText(/No recognized export template is recorded/)
    ).toBeInTheDocument();
    expect(within(section).queryByText('Language not recorded')).not.toBeInTheDocument();
  });

  it('keeps "Unknown" (no hint) for an unrecorded board', () => {
    mockState = {
      data: report({
        groups: [group('board', 'unknown', counts({ noReply: 2 }), null)],
      }),
    };
    render(<InsightsPage />);
    expect(screen.getByText('Unknown')).toBeInTheDocument();
    expect(screen.queryByText('Language not recorded')).not.toBeInTheDocument();
  });

  it('shows the volume nudge even when there are no outcome groups', () => {
    mockState = {
      data: report({
        groups: [],
        nudge: {
          windowHours: 24,
          applications: 5,
          untailored: 3,
          letters: 0,
          nearCopyLetters: 0,
          triggers: ['untailored'],
        },
      }),
    };
    render(<InsightsPage />);
    const nudge = screen.getByRole('region', { name: '5 applications in the last 24 hours' });
    expect(
      within(nudge).getByText('3 of them have no generation recorded in the app.')
    ).toBeInTheDocument();
    expect(screen.getByText('No application outcomes to show')).toBeInTheDocument();
  });

  it('keeps the cached report with an out-of-date note and a retry when a refresh fails', () => {
    mockState = { data: report(), isError: true };
    render(<InsightsPage />);
    expect(screen.getByText('30% positive reply rate')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent(/Out of date/);
    expect(screen.queryByText('Insights could not be loaded')).not.toBeInTheDocument();
    screen.getByRole('button', { name: 'Refresh' }).click();
    expect(refetch).toHaveBeenCalled();
  });

  it('shows when the report was generated', () => {
    mockState = { data: report({ generatedAt: Date.now() - 5 * 60_000 }) };
    render(<InsightsPage />);
    expect(screen.getByText('Updated 5m ago')).toBeInTheDocument();
  });

  it('shows no nudge when the report has none', () => {
    mockState = { data: report() };
    render(<InsightsPage />);
    expect(screen.queryByText(/applications in the last/)).not.toBeInTheDocument();
  });

  it('shows the volume nudge with its real counts and the advice, as plain labelled content', () => {
    mockState = {
      data: report({
        nudge: {
          windowHours: 24,
          applications: 12,
          untailored: 4,
          letters: 8,
          nearCopyLetters: 6,
          triggers: ['nearCopyLetters'],
        },
      }),
    };
    render(<InsightsPage />);
    const nudge = screen.getByRole('region', { name: '12 applications in the last 24 hours' });
    expect(within(nudge).getByText('12 applications in the last 24 hours')).toBeInTheDocument();
    expect(
      within(nudge).getByText(
        '6 of the 8 newest cover letters compared are near-identical to another one.'
      )
    ).toBeInTheDocument();
    expect(within(nudge).getByText(/Tailoring each application/)).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    // only the trigger that held is described
    expect(within(nudge).queryByText(/no tailored/)).not.toBeInTheDocument();
    expect(within(nudge).queryByRole('button')).not.toBeInTheDocument();
  });
});
