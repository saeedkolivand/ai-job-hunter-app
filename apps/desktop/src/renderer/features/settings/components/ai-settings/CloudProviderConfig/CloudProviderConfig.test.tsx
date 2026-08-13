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
 *  - base URL save: a rejected `setProviderSettings` write (the `{error}`-union
 *    narrowing added for review #16) surfaces an error notification instead of
 *    silently swallowing the failure, and an emptied input clears the URL
 *    explicitly (`baseUrl: null`) rather than reading as "unchanged".
 *
 * No QueryClient / AppClientProvider needed — the component's only hooks are
 * useTranslation (stubbed), useNotification (stubbed), and useSaveProviderSettings
 * (stubbed).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

// ── i18n stub ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── @ajh/ui — use the real library, override only useNotification (no
// NotificationProvider mounted in these focused tests) ─────────────────────

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

// ── Service stub — the base_url save now writes via the backend setter (task #16).
// `mutate` invokes the caller's `onError` synchronously so tests can simulate a
// rejected `{error}`-union write without a real QueryClient/mutation lifecycle.

// Succeeds by default; a case that wants the rejection opts in with
// `mockImplementationOnce`. A stub that fails EVERY save meant the payload
// assertions all ran with the component in its error state.
const mockSaveProviderSettings = vi.fn();
const failNextSave = () =>
  mockSaveProviderSettings.mockImplementationOnce(
    (_req: unknown, opts?: { onError?: () => void }) => opts?.onError?.()
  );

// EffortPicker's capability probe — controllable per-test via this mutable
// state object (avoids a real QueryClient/AppClientProvider in these focused
// tests, mirroring the useSaveProviderSettings stub above).
const modelCapsState: { data: { effortLevels: string[] } | undefined } = {
  data: undefined,
};

vi.mock('@/services', () => ({
  useSaveProviderSettings: () => ({ save: mockSaveProviderSettings, isPending: false }),
  useModelCapabilities: () => modelCapsState,
}));

vi.mock('@/store/preferences-store', () => ({
  usePreferencesStore: (selector: (s: { setProviderSettings: () => void }) => unknown) =>
    selector({ setProviderSettings: vi.fn() }),
  useAiProviderConfig: () => undefined,
}));

afterEach(() => {
  vi.clearAllMocks();
  modelCapsState.data = undefined;
});

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

describe('CloudProviderConfig — model dropdown keeps an unlisted stored selection', () => {
  it('shows a stored model that fell out of the curated list, not the placeholder', () => {
    // Regression guard (PR #901 review): a user with a stored model no longer
    // in the curated/live list must not see "Select a model…" as if their
    // config was reset — Dropdown's trigger renders selectedOption?.label,
    // found via options.find(o => o.value === value).
    render(<CloudProviderConfig {...baseProps} providerModel="claude-sonnet-4-6" />);
    expect(screen.getByRole('button', { name: /claude-sonnet-4-6/ })).toBeInTheDocument();
    expect(screen.queryByText('Select a model…')).not.toBeInTheDocument();
  });
});

describe('CloudProviderConfig — base URL save surfaces a rejected write', () => {
  it('shows an error notification (i18n key, not raw error) when the save rejects', async () => {
    const user = userEvent.setup();
    render(
      <CloudProviderConfig {...baseProps} provider="openai-compatible" baseUrlInput="https://x" />
    );

    failNextSave();

    await user.click(screen.getByText('settings.aiProvider.saveUrl'));

    expect(mockSaveProviderSettings).toHaveBeenCalledOnce();
    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'settings.aiProvider.saveUrlFailed',
    });
  });

  it('saves an EMPTIED base URL as an explicit clear, not as "unchanged"', async () => {
    const user = userEvent.setup();
    render(<CloudProviderConfig {...baseProps} provider="openai-compatible" baseUrlInput="" />);

    await user.click(screen.getByText('settings.aiProvider.saveUrl'));

    // `null` is the writer's "clear this field"; `undefined` would mean "keep
    // whatever is stored", which would make the URL impossible to remove.
    expect(mockSaveProviderSettings).toHaveBeenCalledWith(
      expect.objectContaining({ provider: 'openai-compatible', baseUrl: null }),
      expect.anything()
    );
    // The default stub resolves, so this also covers the SUCCESS path: no error
    // notification is raised for a write that worked.
    expect(mockNotify.error).not.toHaveBeenCalled();
  });
});

describe('CloudProviderConfig — model list states (live-model-lists PR)', () => {
  it('shows the real failure message when the fetch failed and there is no cache/stored selection', async () => {
    const onRecheck = vi.fn();
    const user = userEvent.setup();
    render(
      <CloudProviderConfig
        {...baseProps}
        expandedModels={[]}
        expandedModelsError="invalid or unauthorized API key"
        onRecheck={onRecheck}
      />
    );
    expect(screen.getByText('settings.aiModel.fetchFailedTitle')).toBeInTheDocument();
    expect(screen.getByText('invalid or unauthorized API key')).toBeInTheDocument();
    // `ErrorState` carries role="alert" itself — a screen-reader user gets the
    // failure announced without needing to be focused inside the panel.
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /select a model/i })).not.toBeInTheDocument();

    // The presentation alone doesn't prove the retry action is wired up.
    await user.click(screen.getByRole('button', { name: /try again/i }));
    expect(onRecheck).toHaveBeenCalledOnce();
  });

  it('shows a neutral empty state when the fetch succeeded but the catalogue is empty', () => {
    render(<CloudProviderConfig {...baseProps} expandedModels={[]} />);
    expect(screen.getByText('settings.aiModel.emptyTitle')).toBeInTheDocument();
    // `EmptyState` carries role="status" — announced politely, not as an alert.
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('shows a labelled loading indicator while the model list is still loading — not the empty state and not a bare empty dropdown', () => {
    render(<CloudProviderConfig {...baseProps} expandedModels={[]} expandedModelsLoading />);

    // Positive assertion — regression guard for the loading state falling
    // through to an unlabelled `<Dropdown options={[]} />` (indistinguishable
    // from "zero models"), not just the absence of the wrong state.
    const loading = screen.getByRole('status');
    expect(loading).toHaveTextContent('settings.aiModel.loading');
    expect(screen.queryByText('settings.aiModel.emptyTitle')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /select a model/i })).not.toBeInTheDocument();
  });

  it('renders the dropdown plus a cached-list note when models were served from the cache', () => {
    render(
      <CloudProviderConfig
        {...baseProps}
        expandedModels={[{ name: 'gpt-4o' }]}
        expandedModelsCached
        providerModel="gpt-4o"
      />
    );
    expect(screen.getByText('settings.aiModel.cachedNotice')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /gpt-4o/ })).toBeInTheDocument();
  });

  it('preserves an unlisted stored selection even when the live/cached list is empty (no false empty state)', () => {
    render(
      <CloudProviderConfig {...baseProps} expandedModels={[]} providerModel="claude-sonnet-4-6" />
    );
    expect(screen.queryByText('settings.aiModel.emptyTitle')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /claude-sonnet-4-6/ })).toBeInTheDocument();
  });

  it('labels a model option with displayName over name when the provider returns one', () => {
    render(
      <CloudProviderConfig
        {...baseProps}
        expandedModels={[{ name: 'gpt-4o', displayName: 'GPT-4o (Omni)' }]}
        providerModel="gpt-4o"
      />
    );
    expect(screen.getByRole('button', { name: /GPT-4o \(Omni\)/ })).toBeInTheDocument();
  });

  // CodeRabbit #936, Major 2: #935 made the backend list models for
  // `openai-compatible` without a bearer header (LM Studio / vLLM are
  // keyless), and ModelSelector was unblocked to match — but the Settings
  // model-selector block was still gated on `connected` (has a stored key),
  // so a keyless setup could never discover or pick a model here even though
  // nothing about it actually requires a key. It still needs a base URL —
  // this is the "actually configured" case (see the no-baseUrl regression
  // test below for the case it must NOT unblock).
  it('shows the model selector for a keyless openai-compatible provider with a base URL set (connected: false)', () => {
    render(
      <CloudProviderConfig
        {...baseProps}
        provider="openai-compatible"
        connected={false}
        baseUrlInput="http://localhost:1234/v1"
        configuredBaseUrl="http://localhost:1234/v1"
        expandedModels={[{ name: 'local-model' }]}
        providerModel="local-model"
      />
    );
    expect(screen.getByText('settings.aiModel.title')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /local-model/ })).toBeInTheDocument();
  });

  it('shows the loading/error/empty states for a keyless-but-configured openai-compatible provider too', () => {
    const { rerender } = render(
      <CloudProviderConfig
        {...baseProps}
        provider="openai-compatible"
        connected={false}
        baseUrlInput="http://localhost:1234/v1"
        configuredBaseUrl="http://localhost:1234/v1"
        expandedModels={[]}
        expandedModelsLoading
      />
    );
    expect(screen.getByRole('status')).toHaveTextContent('settings.aiModel.loading');

    rerender(
      <CloudProviderConfig
        {...baseProps}
        provider="openai-compatible"
        connected={false}
        baseUrlInput="http://localhost:1234/v1"
        configuredBaseUrl="http://localhost:1234/v1"
        expandedModels={[]}
        expandedModelsError="connection refused"
      />
    );
    expect(screen.getByRole('alert')).toHaveTextContent('connection refused');
  });

  // Finding 3 (PR #937 review): the parent used to compute two DIFFERENT
  // values for "the openai-compatible base URL" — the raw `baseUrlInput` fed
  // straight into this component's own check, vs. a trimmed-or-saved value
  // used everywhere else (the actual model fetch). A user who selects-all and
  // clears the input WITHOUT saving would then lose the picker (and their
  // selected model) while the fetch, reading the other value, kept using the
  // still-saved URL underneath. `configuredBaseUrl` is now the ONLY value fed
  // to `canPickModel` — this pins that `baseUrlInput` (the raw keystroke) no
  // longer has any say in it.
  it('keeps the model selector visible when the input is cleared but the resolved base URL is still saved', () => {
    render(
      <CloudProviderConfig
        {...baseProps}
        provider="openai-compatible"
        connected={false}
        baseUrlInput=""
        configuredBaseUrl="http://localhost:1234/v1"
        expandedModels={[{ name: 'local-model' }]}
        providerModel="local-model"
      />
    );
    expect(screen.getByText('settings.aiModel.title')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /local-model/ })).toBeInTheDocument();
  });

  it('still hides the model selector for a key-required provider with no key (unchanged behavior)', () => {
    render(<CloudProviderConfig {...baseProps} provider="openai" connected={false} />);
    expect(screen.queryByText('settings.aiModel.title')).not.toBeInTheDocument();
  });

  // Privacy regression (fix/no-unconfigured-openai-probe): an openai-compatible
  // row with neither a stored key NOR a base URL — the state #936's carve-out
  // (`connected || provider === 'openai-compatible'`) left unguarded, letting
  // the backend silently fall back to `api.openai.com`. Must stay hidden, same
  // as any other never-configured cloud provider.
  it('hides the model selector for an unconfigured openai-compatible provider (no key, no base URL)', () => {
    render(
      <CloudProviderConfig
        {...baseProps}
        provider="openai-compatible"
        connected={false}
        baseUrlInput=""
      />
    );
    expect(screen.queryByText('settings.aiModel.title')).not.toBeInTheDocument();
  });
});

describe('CloudProviderConfig — reasoning-effort picker is capability-driven', () => {
  it('appears once the backend reports levels for the selected model', () => {
    modelCapsState.data = { effortLevels: ['low', 'medium', 'high'] };
    render(<CloudProviderConfig {...baseProps} provider="openai" providerModel="gpt-5.6" />);
    expect(screen.getByText('settings.aiProvider.reasoningEffort')).toBeInTheDocument();
  });

  it('disappears for a model the backend reports no levels for', () => {
    modelCapsState.data = { effortLevels: [] };
    render(<CloudProviderConfig {...baseProps} provider="openai" providerModel="gpt-4o" />);
    expect(screen.queryByText('settings.aiProvider.reasoningEffort')).not.toBeInTheDocument();
  });

  it('stays hidden while the capability query has not resolved yet', () => {
    modelCapsState.data = undefined;
    render(<CloudProviderConfig {...baseProps} provider="openai" providerModel="gpt-5.6" />);
    expect(screen.queryByText('settings.aiProvider.reasoningEffort')).not.toBeInTheDocument();
  });

  it('never renders for openai-compatible, even if the (stubbed) probe reports levels', () => {
    // The real backend never guesses reasoning support for an unknown
    // openai-compatible gateway catalog (`supports_reasoning_effort` is a
    // hard `false` for it), so `effortLevels` can never be non-empty here in
    // production — but this pins the RENDER-level guard too, since dropping
    // it would resume firing a capability probe on every un-debounced
    // keystroke in the base-URL input for a picker that can never show
    // anything.
    modelCapsState.data = { effortLevels: ['low', 'medium', 'high'] };
    render(
      <CloudProviderConfig {...baseProps} provider="openai-compatible" baseUrlInput="https://x" />
    );
    expect(screen.queryByText('settings.aiProvider.reasoningEffort')).not.toBeInTheDocument();
  });
});
