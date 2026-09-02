/**
 * Pins the dashboard "AI Model" row to `useCanUseAI` (provider-kind-aware),
 * not `system_health.ai.ready` — which only ever probes the LOCAL Ollama
 * daemon, regardless of which provider is actually active. Every case below
 * keeps the Ollama probe reporting `ready: false` (`mockHealth.ai.ready`) to
 * prove the row no longer depends on it.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

import { AISystemStatus } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: vi.fn() }),
}));

vi.mock('@/hooks/use-kind-label-map', () => ({
  useKindLabelMap: () => ({}),
}));

// The seam under test — same shape/module `@/components/ui/ModelSelector`
// mocks elsewhere in the suite (see ApplyByEmailTab.test.tsx).
let mockCanUseAI: { canUse: boolean; reason?: string } = { canUse: true };
let mockSelectedModel = '';
vi.mock('@/components/ui/ModelSelector', () => ({
  useCanUseAI: () => mockCanUseAI,
  useSelectedModel: () => mockSelectedModel,
  useSelectedProvider: () => 'ollama',
}));

// Deliberately always "Ollama unreachable" — the local-only probe this row
// must NOT depend on any more.
const mockHealth = {
  ai: { ready: false, model: null },
  data: { sqlite: true, vector: true },
};
vi.mock('@/services', () => ({
  useSystemHealth: () => ({ data: mockHealth, isFetching: false }),
  useSystemMetrics: () => ({ data: undefined }),
  useWorkerActivity: () => ({ isActive: false, active: 0, queued: 0 }),
}));

function aiDetailText(): string | null | undefined {
  const label = screen.getByText('dashboard.status.aiModel');
  return label.nextElementSibling?.textContent;
}

describe('AISystemStatus — AI-model row reads useCanUseAI, not the Ollama-only probe', () => {
  it('shows the active model as ready even though system_health.ai.ready is false', () => {
    // Regression: before the fix, `h.ai?.ready` (false, from mockHealth above)
    // alone decided this row, so a configured cloud/CLI provider with local
    // Ollama simply not running rendered "AI Model not available".
    mockCanUseAI = { canUse: true };
    mockSelectedModel = 'gpt-4o';
    render(<AISystemStatus />);
    expect(aiDetailText()).toBe('gpt-4o');
  });

  it('falls back to the "ready" label when canUse is true but no model name is resolved (e.g. a CLI-agent default)', () => {
    mockCanUseAI = { canUse: true };
    mockSelectedModel = '';
    render(<AISystemStatus />);
    expect(aiDetailText()).toBe('dashboard.status.ready');
  });

  it.each([
    ['addApiKey', 'aiSetup.addApiKey'],
    ['selectModel', 'aiSetup.selectModel'],
    ['installCli', 'aiSetup.installCli'],
    ['startOllama', 'aiSetup.startOllama'],
    ['healthUnavailable', 'aiSetup.healthUnavailable'],
  ] as const)('renders the %s setup hint, not a generic "not available"', (reason, expectedKey) => {
    mockCanUseAI = { canUse: false, reason };
    mockSelectedModel = '';
    render(<AISystemStatus />);
    expect(aiDetailText()).toBe(expectedKey);
  });

  it('shows "checking" (not an error) while useCanUseAI is still resolving', () => {
    // useCanUseAI's own convention: canUse=false with NO reason means
    // "still resolving" — not "blocked". See its source doc comment.
    //
    // Reached by more than the cold-boot config read: a cloud provider whose
    // KEY query is still in flight also answers reason-less now, so this row
    // must not print "add an API key" for it. That upstream half is pinned in
    // `components/ui/ModelSelector/useCanUseAI.test.tsx` (the real hook,
    // mock-client-backed) — it cannot be pinned here, because this file mocks
    // `@/components/ui/ModelSelector` wholesale and so no case in it can fail
    // from a change inside the hook.
    mockCanUseAI = { canUse: false, reason: undefined };
    mockSelectedModel = '';
    render(<AISystemStatus />);
    expect(aiDetailText()).toBe('dashboard.status.checking');
  });
});
