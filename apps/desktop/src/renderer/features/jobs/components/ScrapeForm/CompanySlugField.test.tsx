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
import { type ComponentProps, createRef, type ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { BoardCatalogEntry } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';
import { type CompanyTypeaheadHandle, NotificationProvider } from '@ajh/ui';

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
      ref={props.ref}
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
    const searchCompanies = vi.fn().mockResolvedValue([
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

  it('commits a typed-but-unentered slug on blur so Start Scrape never drops it', async () => {
    const onChange = vi.fn();
    renderField({ onChange }, { 'discovery.searchCompanies': vi.fn().mockResolvedValue([]) });

    const input = screen.getByTestId(TEST_IDS.jobs.companyTypeahead);
    await userEvent.type(input, 'acme'); // NO Enter
    // Focus leaves the typeahead entirely (e.g. clicking Start Scrape).
    fireEvent.focusOut(input, { relatedTarget: document.body });

    expect(onChange).toHaveBeenCalledWith(['acme']);
  });

  it('flushes a pending slug via commitPending() with NO blur and NO Enter (WebKit-safe)', async () => {
    // The Start-Scrape correctness backstop: WebKit may not blur the input on a
    // sibling-button click, so the submit path calls commitPending() directly.
    const onChange = vi.fn();
    const ref = createRef<CompanyTypeaheadHandle>();
    renderField({ onChange, ref }, { 'discovery.searchCompanies': vi.fn().mockResolvedValue([]) });

    await userEvent.type(screen.getByTestId(TEST_IDS.jobs.companyTypeahead), 'acme');
    act(() => ref.current?.commitPending());

    expect(onChange).toHaveBeenCalledWith(['acme']);
  });

  it('does not double-add when blur AND commitPending both run (idempotent)', async () => {
    const onChange = vi.fn();
    const ref = createRef<CompanyTypeaheadHandle>();
    renderField({ onChange, ref }, { 'discovery.searchCompanies': vi.fn().mockResolvedValue([]) });

    const input = screen.getByTestId(TEST_IDS.jobs.companyTypeahead);
    await userEvent.type(input, 'acme');
    fireEvent.focusOut(input, { relatedTarget: document.body }); // blur-commit clears the query
    act(() => ref.current?.commitPending()); // query already empty → no-op

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(['acme']);
  });

  it('the per-row star toggle fires the setStarred mutation', async () => {
    const searchCompanies = vi.fn().mockResolvedValue([
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

  it('guards a rapid double-click so an in-flight star write fires only once', async () => {
    const searchCompanies = vi.fn().mockResolvedValue([
      {
        atsKind: 'greenhouse',
        slug: 'stripe',
        displayName: 'Stripe',
        seenCount: 5,
        starred: false,
        source: 'scrape',
      },
    ]);
    // A deferred promise keeps the mutation in flight across both clicks, so the
    // second click hits the `isPending` guard instead of firing a stale write.
    let resolveStar: () => void = () => {};
    const setStarred = vi.fn(
      () =>
        new Promise<{ success: true }>((resolve) => {
          resolveStar = () => resolve({ success: true });
        })
    );
    renderField(
      {},
      { 'discovery.searchCompanies': searchCompanies, 'discovery.setStarred': setStarred }
    );

    await userEvent.click(screen.getByTestId(TEST_IDS.jobs.companyTypeahead));
    const star = await screen.findByTestId(TEST_IDS.jobs.companyStarToggle);
    await userEvent.click(star);
    await userEvent.click(star); // in-flight → guarded, no second write

    expect(setStarred).toHaveBeenCalledTimes(1);

    // Settle the mutation so it doesn't leak into the next test.
    await act(async () => {
      resolveStar();
    });
  });

  it('shows an error toast when starring fails (the mutation throws)', async () => {
    const searchCompanies = vi.fn().mockResolvedValue([
      {
        atsKind: 'greenhouse',
        slug: 'stripe',
        displayName: 'Stripe',
        seenCount: 5,
        starred: false,
        source: 'scrape',
      },
    ]);
    // The command RESOLVES an { error } union (never rejects), so useSetStarred
    // narrows it and throws → the field's onError fires notify.error. Guards the
    // silent-failure trap (#756): a failed star must surface, not be swallowed.
    const setStarred = vi.fn().mockResolvedValue({ error: 'boom' });
    renderField(
      {},
      { 'discovery.searchCompanies': searchCompanies, 'discovery.setStarred': setStarred }
    );

    await userEvent.click(screen.getByTestId(TEST_IDS.jobs.companyTypeahead));
    const row = await screen.findByTestId(TEST_IDS.jobs.companySuggestion);
    await userEvent.click(within(row).getByTestId(TEST_IDS.jobs.companyStarToggle));

    expect(await screen.findByText('jobs.discovery.starFailed')).toBeInTheDocument();
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
