/**
 * CloudProviderConfig — focused behaviour tests.
 *
 * Covers:
 *  - connected + not-changing: stored-key badge shows "Change key" + "Remove" buttons.
 *  - clicking "Change key" reveals the password input (change-mode).
 *  - in change-mode with a non-empty apiKeyInput: clicking Save calls onSaveKey.
 *  - in change-mode: the eye-toggle button has an accessible name
 *    (`settings.aiProvider.showKey`) via its aria-label — guards against
 *    accidental removal of the label in future refactors.
 *
 * No QueryClient / AppClientProvider needed — the component's only hooks are
 * useTranslation (stubbed) and useSetProviderSettings (stubbed).
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── i18n stub ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── Service stub — the base_url save now writes via the backend setter (task #16)

vi.mock('@/services', () => ({
  useSetProviderSettings: () => ({ mutate: vi.fn() }),
}));

// ── component under test ───────────────────────────────────────────────────

import { CloudProviderConfig } from './index';

// ── shared props ───────────────────────────────────────────────────────────

const baseProps = {
  provider: 'openai' as const,
  meta: {
    label: 'OpenAI',
    description: '',
    docsUrl: 'https://platform.openai.com',
    color: '',
    models: [],
  },
  connected: true,
  isSaving: false,
  providerModel: '',
  expandedModels: [],
  apiKeyInput: '',
  showKey: false,
  baseUrlInput: '',
  onApiKeyChange: vi.fn(),
  onToggleShowKey: vi.fn(),
  onBaseUrlChange: vi.fn(),
  onSaveKey: vi.fn(),
  onRemoveKey: vi.fn(),
  onSelectModel: vi.fn(),
  onSetActive: vi.fn(),
  isActive: false,
  onOpenDocs: vi.fn(),
};

// ── tests ──────────────────────────────────────────────────────────────────

describe('CloudProviderConfig — connected, not changing', () => {
  it('renders the "Change key" button', () => {
    render(<CloudProviderConfig {...baseProps} />);
    expect(screen.getByText('settings.aiProvider.changeKey')).toBeInTheDocument();
  });

  it('renders the "Remove" button', () => {
    render(<CloudProviderConfig {...baseProps} />);
    expect(screen.getByText('settings.aiProvider.removeKey')).toBeInTheDocument();
  });

  it('does NOT show the password input before entering change-mode', () => {
    const { container } = render(<CloudProviderConfig {...baseProps} />);
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    // password inputs are not role="textbox"; query by type attribute
    expect(container.querySelector('input[type="password"]')).toBeNull();
  });
});

describe('CloudProviderConfig — clicking "Change key" reveals the password input', () => {
  it('keeps change-mode open with an initially empty input (the default connected path)', async () => {
    const user = userEvent.setup();
    // baseProps.apiKeyInput is '' — the real connected state before the user
    // types. The editor must stay open: collapse only fires on the save→done
    // edge, never on first open. (Regression guard for the premature collapse.)
    const { container } = render(<CloudProviderConfig {...baseProps} />);

    await user.click(screen.getByText('settings.aiProvider.changeKey'));

    expect(container.querySelector('input[type="password"]')).not.toBeNull();
  });
});

describe('CloudProviderConfig — save in change-mode calls onSaveKey', () => {
  it('calls onSaveKey when Save is clicked and apiKeyInput is non-empty', async () => {
    const onSaveKey = vi.fn();
    const user = userEvent.setup();
    render(<CloudProviderConfig {...baseProps} apiKeyInput="sk-test" onSaveKey={onSaveKey} />);

    // Enter change-mode
    await user.click(screen.getByText('settings.aiProvider.changeKey'));

    await user.click(screen.getByText('settings.aiProvider.saveKey'));

    expect(onSaveKey).toHaveBeenCalledOnce();
  });
});

describe('CloudProviderConfig — eye-toggle a11y in change-mode', () => {
  it('eye-toggle button has an accessible name', async () => {
    const user = userEvent.setup();
    render(<CloudProviderConfig {...baseProps} />);

    await user.click(screen.getByText('settings.aiProvider.changeKey'));

    // The button carries aria-label={t('settings.aiProvider.showKey')}
    // (line 85 of index.tsx). Guards against accidental removal.
    expect(screen.getByRole('button', { name: 'settings.aiProvider.showKey' })).toBeInTheDocument();
  });
});
