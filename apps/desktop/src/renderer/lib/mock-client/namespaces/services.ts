import type { AppClient } from '../../app-client';
import { noop } from './helpers';

export const serviceNamespaces = (): Pick<
  AppClient,
  'match' | 'geocode' | 'credentials' | 'linkedin' | 'boards' | 'cliAgents' | 'privacy'
> => ({
  match: {
    resume: noop,
    text: noop,
    trimSuggestions: async () => ({ maxPages: 2, lines: [] }),
  },

  geocode: {
    suggest: async () => [],
  },

  credentials: {
    available: async () => false,
  },

  linkedin: {
    connect: noop,
    disconnect: noop,
    getStatus: async () => ({ connected: false }),
    importProfileFromUrl: async () => ({ error: 'not available in mock' }),
    importCookies: async () => ({ outcome: 'NoSession', imported: 0 }),
  },

  boards: {
    catalog: async () => [],
    health: async () => [],
    connect: async () => ({ connected: false }),
    disconnect: noop,
    getStatus: async () => ({ connected: false }),
    importCookies: async () => ({ outcome: 'NoSession', imported: 0 }),
  },

  cliAgents: {
    status: async () => ({ agents: [], npmAvailable: false }),
    redetect: async () => ({ agents: [], npmAvailable: false }),
    install: async () => ({ code: 0, success: true }),
  },

  privacy: {
    signOutAll: noop,
    clearInteractions: noop,
    resetApp: async () => ({ success: true }),
    // Mirrors the Rust default: on, but not yet consented — so a mock-driven
    // test sees the same "does not transmit until asked" state as a fresh install.
    getCrashReporting: async () => ({ enabled: true, consentShown: false }),
    setCrashReporting: async (settings: { enabled: boolean; consentShown: boolean }) => settings,
  },
});
