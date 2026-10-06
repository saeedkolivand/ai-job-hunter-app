/**
 * Shared render helpers for the ExtensionBridgeSection suites (no `.test.` in the
 * name, so vitest does not collect it).
 */
import type { ReactNode } from 'react';
import { beforeEach, type Mock, vi } from 'vitest';
import { QueryClientProvider } from '@tanstack/react-query';
import { render, type RenderResult } from '@testing-library/react';

import type { ActiveAiConfig, ExtensionAiAssistSetting, ExtensionBridgeStatus } from '@ajh/shared';
import { NotificationProvider } from '@ajh/ui';

import { AppClientProvider } from '@/providers/AppClientProvider';
import { createMockClient, makeQueryClient } from '@/test-support';

import { ExtensionBridgeSection } from './index';
import { generateConfig } from './services-stub';

export { generateConfig };

/** Render the section inside real providers over a mock client built from `overrides`. */
export function renderWithClient(
  overrides: Parameters<typeof createMockClient>[0]
): RenderResult & {
  client: ReturnType<typeof createMockClient>;
  queryClient: ReturnType<typeof makeQueryClient>;
} {
  const client = createMockClient(overrides);
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

  const result = render(<ExtensionBridgeSection />, { wrapper: Wrapper });
  return { ...result, client, queryClient };
}

export function renderSection(
  statusPayload: ExtensionBridgeStatus = {
    port: 9712,
    connected: true,
    lastSeenMs: null,
    token: 'tok-abc123',
  },
  regenerateImpl: () => Promise<unknown> = () => Promise.resolve({ token: 'tok-new' }),
  autofillEnabled = false,
  setAutofill: Mock = vi
    .fn()
    .mockImplementation((enabled: boolean) => Promise.resolve({ enabled })),
  aiAssist: ExtensionAiAssistSetting = { enabled: false },
  setAiAssistEnabled: Mock = vi
    .fn()
    .mockImplementation((enabled: boolean) => Promise.resolve({ enabled })),
  // The backend active generation config the "Using: X · Y" label reads
  // (task #16) — `providers` is always present, `activeProvider`/`model`
  // absent until a provider is selected. Default: no active provider.
  activeConfig: ActiveAiConfig = { providers: {} }
): ReturnType<typeof renderWithClient> {
  return renderWithClient({
    'extensionBridge.status': vi.fn().mockResolvedValue(statusPayload),
    'extensionBridge.regenerateToken': vi.fn().mockImplementation(regenerateImpl),
    'extensionBridge.autofillEnabled': vi.fn().mockResolvedValue({ enabled: autofillEnabled }),
    'extensionBridge.setAutofillEnabled': setAutofill,
    'extensionBridge.aiAssistEnabled': vi.fn().mockResolvedValue(aiAssist),
    'extensionBridge.setAiAssistEnabled': setAiAssistEnabled,
    'ai.activeConfig': vi.fn().mockResolvedValue(activeConfig),
  });
}

/** Clipboard stub + shared-mutable-state reset; call once at module top level. */
export function installBridgeHooks() {
  beforeEach(() => {
    generateConfig.current = { provider: 'openai', model: 'gpt-4o' };
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
  });
}
