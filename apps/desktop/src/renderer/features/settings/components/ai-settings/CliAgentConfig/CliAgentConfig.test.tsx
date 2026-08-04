/**
 * CliAgentConfig — model-dropdown orphan-selection guard.
 *
 * Mirrors the CloudProviderConfig fix: `Dropdown`'s trigger renders
 * `selectedOption?.label ?? placeholder` (options.find(o => o.value === value)),
 * so a stored `providerModel` no longer present in the CLI agent's known
 * alias list (e.g. after a curated-list refresh) would fall back to the
 * placeholder and read as a reset config. A synthetic option keeps it selected.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@/store/preferences-store', () => ({
  usePreferencesStore: (selector: (s: { setProviderSettings: () => void }) => unknown) =>
    selector({ setProviderSettings: vi.fn() }),
  useAiProviderConfig: () => undefined,
}));

// EffortPicker's useModelCapabilities — no QueryClient mounted in these
// focused tests, so stub it directly (not exercised by this file's guards).
vi.mock('@/services', () => ({
  useModelCapabilities: () => ({ data: undefined }),
}));

import { CliAgentConfig } from './index';

const baseProps = {
  provider: 'claude-code' as const,
  connected: true,
  expandedModels: [],
  onSelect: vi.fn(),
  onSetActive: vi.fn(),
  isActive: false,
  onInstall: vi.fn(),
  onRecheck: vi.fn(),
};

describe('CliAgentConfig — model dropdown keeps an unlisted stored selection', () => {
  it('shows a curated-list model as the selected label', () => {
    render(<CliAgentConfig {...baseProps} providerModel="sonnet" />);
    expect(screen.getByRole('button', { name: /sonnet/ })).toBeInTheDocument();
  });

  it('still shows a stored model that fell out of the curated list, not the placeholder', () => {
    render(<CliAgentConfig {...baseProps} providerModel="claude-3-5-sonnet-legacy" />);
    expect(screen.getByRole('button', { name: /claude-3-5-sonnet-legacy/ })).toBeInTheDocument();
    expect(screen.queryByText('Select a model…')).not.toBeInTheDocument();
  });
});
