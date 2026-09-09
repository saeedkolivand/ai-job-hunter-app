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

describe('CliAgentConfig — fallback-list labelling (issue #1185)', () => {
  it('shows the fallback notice when every discovered model is source: fallback', () => {
    render(
      <CliAgentConfig
        {...baseProps}
        provider="codex"
        providerModel="gpt-5.5"
        expandedModels={[{ name: 'gpt-5.5', source: 'fallback' }]}
      />
    );
    expect(screen.getByText('models.cli.fallbackList')).toBeInTheDocument();
  });

  it('hides the fallback notice for a live (non-fallback) discovery result', () => {
    render(
      <CliAgentConfig
        {...baseProps}
        provider="codex"
        providerModel="gpt-5.6-terra"
        expandedModels={[{ name: 'gpt-5.6-terra', displayName: 'GPT-5.6-Terra' }]}
      />
    );
    expect(screen.queryByText('models.cli.fallbackList')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /GPT-5\.6-Terra/ })).toBeInTheDocument();
  });

  // claude-code/gemini-cli/antigravity don't implement `discover_models`
  // (Rust default `None`), so their `expandedModels` is always `source:
  // 'fallback'` — the banner is permanent for them, not a failure state. The
  // copy stays neutral ("not the CLI's own live catalogue") rather than
  // asserting the CLI can't ever publish one — it must hold for that case too.
  it('shows the fallback notice for a non-Codex CLI agent (no live discovery to fail)', () => {
    render(
      <CliAgentConfig
        {...baseProps}
        provider="claude-code"
        providerModel="sonnet"
        expandedModels={[{ name: 'sonnet', source: 'fallback' }]}
      />
    );
    expect(screen.getByText('models.cli.fallbackList')).toBeInTheDocument();
  });

  // PR #1187 review: `usingFallbackList` must be driven by the entry's own
  // `source` field, never by "does this backend implement discover_models" —
  // otherwise every non-Codex CLI agent (claude-code/gemini-cli/antigravity,
  // whose `discover_models` is `None` today) would show the banner
  // unconditionally, and a FUTURE backend that gains live discovery would keep
  // showing it too. A non-Codex backend returning plain (non-fallback) entries
  // must hide the banner exactly like Codex's live-discovery case above.
  it('hides the fallback notice for a non-Codex backend returning plain (non-fallback) entries', () => {
    render(
      <CliAgentConfig
        {...baseProps}
        provider="claude-code"
        providerModel="sonnet"
        expandedModels={[{ name: 'sonnet', displayName: 'Sonnet' }]}
      />
    );
    expect(screen.queryByText('models.cli.fallbackList')).not.toBeInTheDocument();
  });

  // A live discovery entry has no reason to omit `displayName`, but the option
  // label must still read as the raw model id rather than rendering blank/`undefined`
  // if it ever does (`m.displayName ?? m.name`).
  it('falls back to the raw model name when a discovered entry has no displayName', () => {
    render(
      <CliAgentConfig
        {...baseProps}
        provider="codex"
        providerModel="gpt-5.5"
        expandedModels={[{ name: 'gpt-5.5' }]}
      />
    );
    expect(screen.getByRole('button', { name: /^gpt-5\.5$/ })).toBeInTheDocument();
  });
});
