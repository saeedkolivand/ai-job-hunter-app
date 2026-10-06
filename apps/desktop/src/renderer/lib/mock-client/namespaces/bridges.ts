import type { EmailWatchConnectRequest } from '@ajh/shared';

import type { AppClient } from '../../app-client';
import { unsub } from './helpers';

export const bridgeNamespaces = (): Pick<AppClient, 'extensionBridge' | 'emailWatch'> => ({
  extensionBridge: {
    status: async () => ({
      port: 47615,
      connected: false,
      lastSeenMs: null,
      token: 'mock-token',
    }),
    regenerateToken: async () => ({ token: 'mock-token' }),
    autofillEnabled: async () => ({ enabled: false }),
    setAutofillEnabled: async (enabled: boolean) => ({ enabled }),
    aiAssistEnabled: async () => ({ enabled: false }),
    setAiAssistEnabled: async (enabled: boolean) => ({ enabled }),
    autoTrackEnabled: async () => ({ enabled: false }),
    setAutoTrackEnabled: async (enabled: boolean) => ({ enabled }),
    onChanged: unsub,
  },

  // autoWriteEnabled defaults to false — matches the real backend default
  // (opt-in only, after five security rounds on the sender-authentication
  // gate's known-imperfect check).
  emailWatch: {
    status: async () => ({ connected: false, enabled: false, autoWriteEnabled: false }),
    connect: async ({ address }: EmailWatchConnectRequest) => ({
      connected: true,
      address,
      enabled: false,
      autoWriteEnabled: false,
    }),
    disconnect: async () => ({ connected: false, enabled: false, autoWriteEnabled: false }),
    setEnabled: async (enabled: boolean) => ({
      connected: false,
      enabled,
      autoWriteEnabled: false,
    }),
    setAutoWriteEnabled: async (autoWriteEnabled: boolean) => ({
      connected: false,
      enabled: false,
      autoWriteEnabled,
    }),
    checkNow: async () => ({ connected: false, enabled: false, autoWriteEnabled: false }),
  },
});
