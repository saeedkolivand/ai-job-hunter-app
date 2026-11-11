/**
 * SearxngField — the SearXNG-compatible server URL: shows the saved value, saves a new one,
 * clears on an empty save, and surfaces the backend's validation error inline.
 *
 * The REAL `useSearchSettings`/`useSetSearchSettings` run against a fake `AppClient`, so the
 * `{ error }` -> throw -> `onError` path is exercised, not stubbed. i18n returns the key.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

import type { AppClient } from '@/lib/app-client';
import { withProviders } from '@/test-support';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const mockNotify = {
  open: vi.fn(),
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
  destroy: vi.fn(),
};
vi.mock('@ajh/ui', async (importOriginal) => {
  const actual = await importOriginal<typeof AjhUi>();
  return { ...actual, useNotification: () => mockNotify };
});

const searchSettings = vi.fn();
const setSearchSettings = vi.fn();

import { SearxngField } from './SearxngField';

const LABEL = 'settings.companyResearch.searxngLabel';

function renderField() {
  const client = { ai: { searchSettings, setSearchSettings } } as unknown as AppClient;
  return render(<SearxngField />, { wrapper: withProviders(client) });
}

describe('SearxngField', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    searchSettings.mockResolvedValue({ searxngUrl: 'https://search.example.com' });
    setSearchSettings.mockImplementation(async ({ searxngUrl }) => ({ searxngUrl }));
  });

  it('renders the saved URL', async () => {
    renderField();
    expect(await screen.findByDisplayValue('https://search.example.com')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /save/i })).toBeDisabled();
  });

  it('saves a new URL', async () => {
    const user = userEvent.setup();
    renderField();
    const input = await screen.findByDisplayValue('https://search.example.com');
    await user.clear(input);
    await user.type(input, 'https://searx.internal:8080');
    await user.click(screen.getByRole('button', { name: /save/i }));

    await waitFor(() =>
      expect(setSearchSettings).toHaveBeenCalledWith({ searxngUrl: 'https://searx.internal:8080' })
    );
    expect(mockNotify.success).toHaveBeenCalledWith({
      message: 'settings.companyResearch.searxngSaved',
    });
  });

  it('clears the setting when the field is saved empty', async () => {
    const user = userEvent.setup();
    renderField();
    const input = await screen.findByDisplayValue('https://search.example.com');
    await user.clear(input);
    await user.click(screen.getByRole('button', { name: /save/i }));

    await waitFor(() => expect(setSearchSettings).toHaveBeenCalledWith({ searxngUrl: null }));
    expect(mockNotify.success).toHaveBeenCalledWith({
      message: 'settings.companyResearch.searxngRemoved',
    });
  });

  it("shows the server's validation error inline and keeps the draft", async () => {
    setSearchSettings.mockResolvedValue({ error: 'URL must start with http:// or https://' });
    const user = userEvent.setup();
    renderField();
    const input = await screen.findByDisplayValue('https://search.example.com');
    await user.clear(input);
    await user.type(input, 'ftp://nope');
    await user.click(screen.getByRole('button', { name: /save/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'URL must start with http:// or https://'
    );
    expect(screen.getByLabelText(LABEL)).toHaveValue('ftp://nope');
    expect(mockNotify.success).not.toHaveBeenCalled();
  });
});
