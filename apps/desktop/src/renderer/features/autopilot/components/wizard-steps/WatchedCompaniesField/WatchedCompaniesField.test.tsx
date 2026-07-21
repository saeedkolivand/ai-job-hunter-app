/**
 * WatchedCompaniesField (ADR-030 §e) — the "My watched ATS companies" target
 * toggle in the wizard board step.
 *
 * Covers: toggling the Switch flips the form's `watchedCompaniesOnly`; the
 * empty-state hint renders when the toggle is on but nothing is starred; and the
 * inline unstar action fires the setStarred mutation.
 */
import type { ReactNode } from 'react';
import { FormProvider, useForm, useWatch } from 'react-hook-form';
import { describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';
import { NotificationProvider } from '@ajh/ui';

import type { WizardState } from '@/features/autopilot/types';
import type { AppClient } from '@/lib/app-client';
import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { WatchedCompaniesField } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

/** Shows the live form value so the toggle's effect is observable. */
function Probe() {
  const value = useWatch<WizardState>({ name: 'watchedCompaniesOnly' });
  return <output data-testid="watched-only-value">{String(value)}</output>;
}

function Harness({
  defaultOnly = false,
  client,
  children,
}: {
  defaultOnly?: boolean;
  client: AppClient;
  children: ReactNode;
}) {
  const methods = useForm<WizardState>({ defaultValues: { watchedCompaniesOnly: defaultOnly } });
  const queryClient = makeQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <AppClientProvider client={client}>
        <NotificationProvider>
          <FormProvider {...methods}>
            {children}
            <Probe />
          </FormProvider>
        </NotificationProvider>
      </AppClientProvider>
    </QueryClientProvider>
  );
}

function renderField(
  opts: { defaultOnly?: boolean; overrides?: Record<string, (...args: never[]) => unknown> } = {}
) {
  const client = createMockClient(opts.overrides ?? {});
  return {
    client,
    ...render(
      <Harness defaultOnly={opts.defaultOnly} client={client}>
        <WatchedCompaniesField />
      </Harness>
    ),
  };
}

describe('WatchedCompaniesField', () => {
  it('toggling the switch sets watchedCompaniesOnly on the form', async () => {
    renderField();
    expect(screen.getByTestId('watched-only-value')).toHaveTextContent('false');
    await userEvent.click(screen.getByRole('switch'));
    await waitFor(() => expect(screen.getByTestId('watched-only-value')).toHaveTextContent('true'));
  });

  it('renders the empty hint when the toggle is on but nothing is starred', async () => {
    renderField({
      defaultOnly: true,
      overrides: { 'discovery.watched': vi.fn().mockResolvedValue([]) },
    });
    expect(
      await screen.findByText('autopilot.wizard.target.watched.emptyHint')
    ).toBeInTheDocument();
  });

  it('lists watched companies and unstar fires the setStarred mutation', async () => {
    const setStarred = vi.fn().mockResolvedValue({ success: true });
    renderField({
      defaultOnly: true,
      overrides: {
        'discovery.watched': vi.fn().mockResolvedValue([
          {
            atsKind: 'greenhouse',
            slug: 'stripe',
            displayName: 'Stripe',
            seenCount: 5,
            starred: true,
            source: 'scrape',
          },
        ]),
        'discovery.setStarred': setStarred,
      },
    });

    // Wait for the watched list, then click the unstar control by accessible name.
    await screen.findByText('Stripe');
    await userEvent.click(
      screen.getByRole('button', { name: 'autopilot.wizard.target.watched.unstar' })
    );

    await waitFor(() =>
      expect(setStarred).toHaveBeenCalledWith({
        atsKind: 'greenhouse',
        slug: 'stripe',
        starred: false,
      })
    );
  });

  it('shows an error toast when unstarring fails (the mutation throws)', async () => {
    // The command RESOLVES an { error } union → useSetStarred narrows + throws →
    // the field's onError fires notify.error (no silent failure — #756).
    const setStarred = vi.fn().mockResolvedValue({ error: 'boom' });
    renderField({
      defaultOnly: true,
      overrides: {
        'discovery.watched': vi.fn().mockResolvedValue([
          {
            atsKind: 'greenhouse',
            slug: 'stripe',
            displayName: 'Stripe',
            seenCount: 5,
            starred: true,
            source: 'scrape',
          },
        ]),
        'discovery.setStarred': setStarred,
      },
    });

    // Wait for the watched list, then click the unstar control by accessible name.
    await screen.findByText('Stripe');
    await userEvent.click(
      screen.getByRole('button', { name: 'autopilot.wizard.target.watched.unstar' })
    );

    expect(await screen.findByText('jobs.discovery.starFailed')).toBeInTheDocument();
  });

  it('carries the watched-companies test id for the wizard step', () => {
    renderField();
    expect(screen.getByTestId(TEST_IDS.autopilot.watchedCompaniesToggle)).toBeInTheDocument();
  });
});
