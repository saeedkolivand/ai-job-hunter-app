/**
 * EffortPicker — the shared reasoning-effort dropdown behind CliAgentConfig,
 * CloudProviderConfig, and OllamaConfig.
 *
 * Covers the visibility gate (hidden until the backend reports non-empty
 * `effortLevels` for this exact model — a per-model lookup, never a
 * hardcoded per-provider list) and the write path (selecting a level calls
 * `setProviderSettings`).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

const modelCapsState: { data: { effortLevels: string[] } | undefined } = {
  data: undefined,
};

vi.mock('@/services', () => ({
  useModelCapabilities: () => modelCapsState,
}));

const setProviderSettings = vi.fn();

const providerConfigState: {
  data: { providers?: Record<string, { effort?: string }> } | undefined;
} = { data: undefined };

vi.mock('@/store/preferences-store', () => ({
  usePreferencesStore: (
    selector: (s: { setProviderSettings: typeof setProviderSettings }) => unknown
  ) => selector({ setProviderSettings }),
  useAiProviderConfig: () => providerConfigState.data,
}));

afterEach(() => {
  vi.clearAllMocks();
  modelCapsState.data = undefined;
  providerConfigState.data = undefined;
});

import { EffortPicker } from './index';

describe('EffortPicker', () => {
  it('renders nothing while the capability query is still pending', () => {
    modelCapsState.data = undefined;
    const { container } = render(<EffortPicker provider="openai" model="gpt-5.6" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when the backend reports no levels for this model', () => {
    modelCapsState.data = { effortLevels: [] };
    const { container } = render(<EffortPicker provider="openai" model="gpt-4o" />);
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the dropdown once the backend reports levels for this model', () => {
    modelCapsState.data = { effortLevels: ['low', 'medium', 'high'] };
    render(<EffortPicker provider="openai" model="gpt-5.6" />);
    expect(screen.getByText('settings.aiProvider.reasoningEffort')).toBeInTheDocument();
  });

  it('renders exactly the levels the backend returns — no hardcoded list', async () => {
    // A gemini-3-pro-preview-shaped response: only two levels, per Google's
    // per-model table — proves the picker doesn't fall back to a static
    // low/medium/high triplet.
    modelCapsState.data = { effortLevels: ['low', 'high'] };
    const user = userEvent.setup();
    render(<EffortPicker provider="gemini" model="gemini-3-pro-preview" />);

    await user.click(screen.getByRole('button', { name: /settings\.aiProvider\.effortDefault/ }));

    expect(screen.getByText('low')).toBeInTheDocument();
    expect(screen.getByText('high')).toBeInTheDocument();
    expect(screen.queryByText('medium')).not.toBeInTheDocument();
  });

  it('writes the picked level via setProviderSettings', async () => {
    modelCapsState.data = { effortLevels: ['low', 'medium', 'high'] };
    const user = userEvent.setup();
    render(<EffortPicker provider="openai" model="gpt-5.6" />);

    await user.click(screen.getByRole('button', { name: /settings\.aiProvider\.effortDefault/ }));
    await user.click(screen.getByText('high'));

    expect(setProviderSettings).toHaveBeenCalledWith('openai', { effort: 'high' });
  });

  it('shows a previously-persisted effort value as selected, not the default placeholder', () => {
    modelCapsState.data = { effortLevels: ['low', 'medium', 'high'] };
    providerConfigState.data = { providers: { openai: { effort: 'high' } } };
    render(<EffortPicker provider="openai" model="gpt-5.6" />);

    // The trigger shows the persisted level's own label, not the
    // "effortDefault" placeholder — proves the picker actually reads the
    // saved config on mount instead of always starting blank.
    expect(screen.getByRole('button', { name: 'high' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /settings\.aiProvider\.effortDefault/ })
    ).not.toBeInTheDocument();
  });
});
