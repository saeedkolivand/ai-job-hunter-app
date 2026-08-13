/**
 * StageOverrideEditor — what the editor SENDS.
 *
 * The payload is the whole contract with the backend, and every field in it is
 * separately refusable server-side, so each branch is asserted on the argument
 * `setStageOverride` receives rather than on what the form looks like.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as AjhUi from '@ajh/ui';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) =>
      params ? `${key} ${Object.values(params).join(' ')}` : key,
  }),
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

const setStageOverrideMutate = vi.fn();
const providerModels: { data: { models: { name: string }[] } } = { data: { models: [] } };

vi.mock('@/services', () => ({
  useSetStageOverride: () => ({ mutate: setStageOverrideMutate, isPending: false }),
  useListProviderModels: () => providerModels,
}));

import { StageOverrideEditor } from './StageOverrideEditor';

const baseProps = {
  id: 'stage-editor-draft',
  stage: 'draft' as const,
  ollamaModels: [{ name: 'qwen3:8b' }, { name: 'llama3.2:1b' }],
  isConfigured: () => true,
  onDone: vi.fn(),
};

/** Pick an option out of the @ajh/ui Dropdown named by `label`. */
const pick = async (user: ReturnType<typeof userEvent.setup>, labelKey: string, option: string) => {
  await user.click(screen.getByLabelText(labelKey));
  await user.click(await screen.findByRole('option', { name: option }));
};

const savedPayload = () => setStageOverrideMutate.mock.calls[0]?.[0];

afterEach(() => {
  vi.clearAllMocks();
  providerModels.data = { models: [] };
});

describe('StageOverrideEditor — the context-window switch', () => {
  it('sends the window only when the switch is ON', async () => {
    const user = userEvent.setup();
    render(
      <StageOverrideEditor
        {...baseProps}
        override={{ provider: 'ollama', model: 'qwen3:8b', contextWindow: 16_384 }}
        fallbackProvider="ollama"
      />
    );

    await user.click(screen.getByText('settings.ai.stages.editor.save'));

    expect(savedPayload()).toMatchObject({ stage: 'draft', contextWindow: 16_384 });
  });

  it('omits the window entirely when the switch is OFF', async () => {
    const user = userEvent.setup();
    render(
      <StageOverrideEditor
        {...baseProps}
        override={{ provider: 'ollama', model: 'qwen3:8b', contextWindow: 16_384 }}
        fallbackProvider="ollama"
      />
    );

    // Turning it off means "use the provider's own default", which is an
    // ABSENT field — sending the number anyway pins a window the user removed.
    await user.click(screen.getByRole('switch'));
    await user.click(screen.getByText('settings.ai.stages.editor.save'));

    expect(savedPayload()).toMatchObject({ stage: 'draft', model: 'qwen3:8b' });
    expect(savedPayload()?.contextWindow).toBeUndefined();
  });

  it('starts with the switch off for an override that has no window', () => {
    render(
      <StageOverrideEditor
        {...baseProps}
        override={{ provider: 'ollama', model: 'qwen3:8b' }}
        fallbackProvider="ollama"
      />
    );

    expect(screen.getByRole('switch')).not.toBeChecked();
    expect(screen.getByText('settings.ai.stages.editor.windowDefault')).toBeVisible();
  });
});

describe('StageOverrideEditor — changing provider', () => {
  it('clears the model, because a model id belongs to one family', async () => {
    const user = userEvent.setup();
    render(
      <StageOverrideEditor
        {...baseProps}
        override={{ provider: 'ollama', model: 'qwen3:8b' }}
        fallbackProvider="ollama"
      />
    );

    await pick(user, 'settings.ai.stages.editor.provider', 'OpenAI');

    // Carrying `qwen3:8b` to OpenAI is exactly what `validate_model` rejects.
    const modelDropdown = screen.getByLabelText('settings.ai.stages.editor.model');
    expect(within(modelDropdown).queryByText('qwen3:8b')).toBeNull();
    // With no model, saving is blocked rather than sending a doomed payload.
    expect(screen.getByText('settings.ai.stages.editor.save')).toBeDisabled();
  });
});

describe('StageOverrideEditor — when saving is allowed', () => {
  it('allows a CLI agent with no model — it runs its own default', async () => {
    const user = userEvent.setup();
    render(<StageOverrideEditor {...baseProps} fallbackProvider="claude-code" />);

    await user.click(screen.getByText('settings.ai.stages.editor.save'));

    expect(savedPayload()).toMatchObject({ stage: 'draft', provider: 'claude-code' });
    expect(savedPayload()?.model).toBeUndefined();
  });

  it('blocks saving against an unconfigured provider, and says why', () => {
    render(
      <StageOverrideEditor {...baseProps} fallbackProvider="openai" isConfigured={() => false} />
    );

    expect(screen.getByText('settings.ai.stages.editor.save')).toBeDisabled();
    expect(
      screen.getByText(/settings\.ai\.stages\.editor\.providerUnconfigured OpenAI/)
    ).toBeVisible();
  });
});

describe('StageOverrideEditor — a stored model missing from the catalogue', () => {
  it('keeps it selectable instead of showing the placeholder', () => {
    providerModels.data = { models: [{ name: 'gpt-5.1' }] };
    render(
      <StageOverrideEditor
        {...baseProps}
        override={{ provider: 'openai', model: 'gpt-4o-retired' }}
        fallbackProvider="openai"
      />
    );

    // The stored choice survives a catalogue that no longer lists it — losing
    // it silently would look like the override had been forgotten.
    expect(screen.getByLabelText('settings.ai.stages.editor.model')).toHaveTextContent(
      'gpt-4o-retired'
    );
  });
});

describe('StageOverrideEditor — a rejected write', () => {
  it('surfaces the server’s own reason', async () => {
    setStageOverrideMutate.mockImplementationOnce((_req, opts?: { onError?: (e: Error) => void }) =>
      opts?.onError?.(new Error('context window out of range'))
    );
    const user = userEvent.setup();
    render(
      <StageOverrideEditor
        {...baseProps}
        override={{ provider: 'ollama', model: 'qwen3:8b' }}
        fallbackProvider="ollama"
      />
    );

    await user.click(screen.getByText('settings.ai.stages.editor.save'));

    expect(mockNotify.error).toHaveBeenCalledWith({
      message: 'settings.ai.stages.saveFailed context window out of range',
    });
  });
});
