/**
 * AboutTab — export diagnostics relocation guard.
 *
 * The export-diagnostics caption + button used to live here and moved to
 * DeveloperPreferences (see DeveloperPreferences.test.tsx). Covers:
 *  1. No export-diagnostics button remains in AboutTab.
 *  2. The donate section still renders, ending with the PayPal link.
 */
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { AboutTab } from './AboutTab';

function renderAboutTab() {
  const client = createMockClient();
  const queryClient = makeQueryClient();

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <AppClientProvider client={client}>{children}</AppClientProvider>
      </QueryClientProvider>
    );
  }

  return render(<AboutTab />, { wrapper: Wrapper });
}

describe('AboutTab', () => {
  it('no longer renders the export diagnostics control', () => {
    renderAboutTab();

    expect(screen.queryByText(/export diagnostics/i)).not.toBeInTheDocument();
  });

  it('still renders the donate links, ending with Send a tip via PayPal', () => {
    renderAboutTab();

    expect(screen.getByRole('button', { name: /send a tip via paypal/i })).toBeInTheDocument();
  });
});
