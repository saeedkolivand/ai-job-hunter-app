/**
 * DeveloperPreferences — export diagnostics control tests.
 *
 * The export-diagnostics caption + button used to live in AboutTab (under the
 * "Fund the hunt" card) and moved here. Covers:
 *  1. The caption + button render in DeveloperPreferences.
 *  2. Clicking the button, saving a destination, and a successful export
 *     surfaces the success notification (re-namespaced i18n key).
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

const mockSave = vi.fn();
const mockRevealItemInDir = vi.fn();

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...args: unknown[]) => mockSave(...args) }));
vi.mock('@tauri-apps/plugin-opener', () => ({
  revealItemInDir: (...args: unknown[]) => mockRevealItemInDir(...args),
}));

vi.mock('@/store/preferences-store', () => ({
  useDebugMode: () => false,
  usePreferencesStore: (selector: (s: { setDebugMode: () => void }) => unknown) =>
    selector({ setDebugMode: vi.fn() }),
}));

// Component import deferred until after the mocks above are hoisted.
import { DeveloperPreferences } from './index';

function renderDeveloperPreferences(exportDiagnostics = vi.fn().mockResolvedValue(undefined)) {
  const client = createMockClient({ 'support.exportDiagnostics': exportDiagnostics });
  const queryClient = makeQueryClient();

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <AppClientProvider client={client}>
          <NotificationProvider>{children}</NotificationProvider>
        </AppClientProvider>
      </QueryClientProvider>
    );
  }

  return render(<DeveloperPreferences />, { wrapper: Wrapper });
}

beforeEach(() => {
  mockSave.mockReset();
  mockRevealItemInDir.mockResolvedValue(undefined);
});

describe('DeveloperPreferences — export diagnostics', () => {
  it('renders the export diagnostics caption and button (moved from AboutTab)', () => {
    renderDeveloperPreferences();

    expect(
      screen.getByText(
        'Save a redacted diagnostics bundle (system info, crash log, app logs) to attach to a bug report.'
      )
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /export diagnostics/i })).toBeInTheDocument();
  });

  it('shows the success notification after a chosen destination exports successfully', async () => {
    mockSave.mockResolvedValue('C:/diagnostics/ajh-diagnostics-2026-01-01.zip');
    const exportDiagnostics = vi.fn().mockResolvedValue({ success: true, path: 'ok' });
    renderDeveloperPreferences(exportDiagnostics);

    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /export diagnostics/i }));

    await waitFor(() => expect(exportDiagnostics).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      expect(screen.getByText('Diagnostics bundle saved.')).toBeInTheDocument();
    });
  });

  it('does not export when the save dialog is dismissed (no destination chosen)', async () => {
    mockSave.mockResolvedValue(null);
    const exportDiagnostics = vi.fn().mockResolvedValue({ success: true, path: 'ok' });
    renderDeveloperPreferences(exportDiagnostics);

    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /export diagnostics/i }));

    await waitFor(() => expect(mockSave).toHaveBeenCalledTimes(1));
    expect(exportDiagnostics).not.toHaveBeenCalled();
  });
});
