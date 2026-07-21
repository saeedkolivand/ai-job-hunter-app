/**
 * CompanySlugField (ADR-030) — the ATS slug typeahead that replaced the old
 * comma-separated company input.
 *
 * Covers, against a real service layer (createMockClient):
 *   - merged suggestions: server-discovered rows + the selected boards' curated seeds
 *   - free-text add still works (unknown slug → chip → submitted companies)
 *   - the per-row star toggle fires the setStarred mutation
 *   - empty state renders the "what is a slug?" SetupHint
 *   - removing a chip updates the submitted companies array
 */
import type { ComponentProps, ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { BoardCatalogEntry } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { CompanySlugField } from './CompanySlugField';

// t() returns the key so assertions don't depend on i18n init.
vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const LEVER_BOARD: BoardCatalogEntry = {
  id: 'lever',
  displayName: 'Lever',
  mode: 'http',
  auth: 'guest',
  listed: true,
  requiresCompany: true,
  seededCompanies: ['ramp'],
};

function renderField(
  props: Partial<ComponentProps<typeof CompanySlugField>>,
  overrides: Record<string, (...args: never[]) => unknown> = {}
) {
  const client = createMockClient(overrides);
  const queryClient = makeQueryClient();
  const ui: ReactElement = (
    <CompanySlugField
      companies={props.companies ?? []}
      onChange={props.onChange ?? vi.fn()}
      seededBoards={props.seededBoards ?? []}
      disabled={props.disabled}
    />
  );
  return {
    client,
    ...render(
      <QueryClientProvider client={queryClient}>
        <AppClientProvider client={client}>
          <NotificationProvider>{ui}</NotificationProvider>
        </AppClientProvider>
      </QueryClientProvider>
    ),
  };
}

describe('CompanySlugField', () => {
  it('merges server-discovered rows with the selected boards curated seeds', async () => {
    const searchCompanies = vi
      .fn()
      .mockResolvedValue([
        {
          atsKind: 'greenhouse',
          slug: 'stripe',
          displayName: 'Stripe',
          seenCount: 5,
          starred: false,
          source: 'scrape',
        },
      ]);
    renderField({ seededBoards: [LEVER_BOARD] }, { 'discovery.searchCompanies': searchCompanies });

    await userEvent.click(screen.getByTestId(TEST_IDS.jobs.companyTypeahead));

    // discovered row
    await waitFor(() => expect(screen.getByText('Stripe')).toBeInTheDocument());
    // curated seed row
    expect(screen.getByText('ramp')).toBeInTheDocument();
  });

  it('adds free text as a chip and submits it (unknown slug is never a dead end)', async () => {
    const onChange = vi.fn();
    renderField({ onChange }, { 'discovery.searchCompanies': vi.fn().mockResolvedValue([]) });

    const input = screen.getByTestId(TEST_IDS.jobs.companyTypeahead);
    await userEvent.type(input, 'acme');
    await userEvent.keyboard('{Enter}');

    expect(onChange).toHaveBeenCalledWith(['acme']);
  });

  it('the per-row star toggle fires the setStarred mutation', async () => {
    const searchCompanies = vi
      .fn()
      .mockResolvedValue([
        {
          atsKind: 'greenhouse',
          slug: 'stripe',
          displayName: 'Stripe',
          seenCount: 5,
          starred: false,
          source: 'scrape',
        },
      ]);
    const setStarred = vi.fn().mockResolvedValue({ success: true });
    renderField(
      {},
      { 'discovery.searchCompanies': searchCompanies, 'discovery.setStarred': setStarred }
    );

    await userEvent.click(screen.getByTestId(TEST_IDS.jobs.companyTypeahead));
    const row = await screen.findByTestId(TEST_IDS.jobs.companySuggestion);
    await userEvent.click(within(row).getByTestId(TEST_IDS.jobs.companyStarToggle));

    await waitFor(() =>
      expect(setStarred).toHaveBeenCalledWith({
        atsKind: 'greenhouse',
        slug: 'stripe',
        starred: true,
      })
    );
  });

  it('renders the slug SetupHint when there are no suggestions', async () => {
    renderField({}, { 'discovery.searchCompanies': vi.fn().mockResolvedValue([]) });
    await userEvent.click(screen.getByTestId(TEST_IDS.jobs.companyTypeahead));
    expect(await screen.findByText('jobs.discovery.slugHint')).toBeInTheDocument();
  });

  it('removing a chip updates the submitted companies array', async () => {
    const onChange = vi.fn();
    renderField({ companies: ['stripe'], onChange });
    const chip = screen.getByTestId(TEST_IDS.jobs.companyChip);
    await userEvent.click(within(chip).getByRole('button'));
    expect(onChange).toHaveBeenCalledWith([]);
  });
});
