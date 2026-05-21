import { createContext, type ReactNode, useContext } from 'react';

import type { RuntimeHealth } from '@ajh/shared';

import { useSystemHealth } from '@/services/use-system';

// ── Capability shape ─────────────────────────────────────────────────────────

export interface Capabilities {
  /** AI runtime (Ollama) status */
  ai: {
    ready: boolean;
    model: string | undefined;
  };
  /** Data layer (SQLite + LanceDB) status */
  data: {
    ready: boolean;
    sqlite: boolean;
    vector: boolean;
  };
  /** Worker thread pool */
  workers: {
    active: number;
    idle: number;
  };
  /** Whether the health check has completed at least once */
  initialized: boolean;
}

const DEFAULT: Capabilities = {
  ai: { ready: false, model: undefined },
  data: { ready: false, sqlite: false, vector: false },
  workers: { active: 0, idle: 0 },
  initialized: false,
};

// ── Context ──────────────────────────────────────────────────────────────────

const CapabilityContext = createContext<Capabilities>(DEFAULT);

function parseHealth(health: RuntimeHealth | undefined): Capabilities {
  if (!health) return DEFAULT;
  return {
    ai: {
      ready: !!health.ai?.ready,
      model: (health.ai as { ready: boolean; model?: string } | undefined)?.model,
    },
    data: {
      ready: !!health.data?.ready,
      sqlite: !!(health.data as { ready: boolean; sqlite?: boolean } | undefined)?.sqlite,
      vector: !!(health.data as { ready: boolean; vector?: boolean } | undefined)?.vector,
    },
    workers: {
      active: (health.workers as { active?: number } | undefined)?.active ?? 0,
      idle: (health.workers as { idle?: number } | undefined)?.idle ?? 0,
    },
    initialized: true,
  };
}

// ── Provider ─────────────────────────────────────────────────────────────────

interface CapabilityProviderProps {
  children: ReactNode;
}

/**
 * Single source of truth for system health across the entire app.
 *
 * Replaces the three independent health pollers that previously lived in:
 *   - Sidebar (every 15s)
 *   - StatusBar (every 5s)
 *   - Per-page checks in ai-generate, analyze, etc.
 *
 * Uses useSystemHealth which polls at 15s via React Query.
 * All consumers get the same cached value — zero duplicate IPC calls.
 */
export function CapabilityProvider({ children }: CapabilityProviderProps) {
  const { data: health } = useSystemHealth();
  const caps = parseHealth(health as RuntimeHealth | undefined);

  return <CapabilityContext.Provider value={caps}>{children}</CapabilityContext.Provider>;
}

// ── Consumer hooks ────────────────────────────────────────────────────────────

export const useCapabilities = () => useContext(CapabilityContext);

export const useAICapability = () => useContext(CapabilityContext).ai;
export const useDataCapability = () => useContext(CapabilityContext).data;
export const useWorkerCapability = () => useContext(CapabilityContext).workers;
