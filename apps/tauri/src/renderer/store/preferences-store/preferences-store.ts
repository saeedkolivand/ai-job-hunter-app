import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import type {
  AiProvider,
  LocalModelLimits,
  PerProviderSettings,
  Preferences,
  PromptQuality,
} from '../preferences-schema';

// Migration function to handle version updates
const STORE_VERSION = 3;

const migratePreferences = (state: Record<string, unknown>, version: number): Preferences => {
  // v0 → v1: baseline
  if (version < 1) {
    state = { ...state, version: 1, lastUpdated: new Date().toISOString() };
  }

  // v2 → v3: add promptQuality default
  if (version < 3) {
    state = { ...state, promptQuality: 'auto', version: 3, lastUpdated: new Date().toISOString() };
  }

  // v1 → v2: flatten { provider, model, baseUrl } → { activeProvider, providers: { … } }
  if (version < 2) {
    const old = state.aiProviderConfig as
      | { provider?: string; model?: string; baseUrl?: string }
      | undefined;
    if (old && 'provider' in old) {
      const p = old.provider ?? 'ollama';
      state = {
        ...state,
        aiProviderConfig: {
          activeProvider: p,
          providers: { [p]: { model: old.model ?? '', baseUrl: old.baseUrl } },
        },
        version: 2,
        lastUpdated: new Date().toISOString(),
      };
    }
  }

  return state as Preferences;
};

// Default preferences
const defaultPreferences: Preferences = {
  version: 1,
  language: 'en',
  outputTone: 'professional',
  performanceMode: 'balanced',
  promptQuality: 'auto',
  debugMode: false,
  onboardingCompleted: false,
  contactPromptSeen: false,
  lastUpdated: new Date().toISOString(),
};

// Create the preferences store
interface PreferencesActions {
  setUserName: (userName: string) => void;
  setLanguage: (language: string) => void;
  setAIModel: (aiModel: Preferences['aiModel']) => void;
  setAiProviderConfig: (config: Preferences['aiProviderConfig']) => void;
  setActiveProvider: (provider: AiProvider) => void;
  setProviderSettings: (provider: AiProvider, settings: Partial<PerProviderSettings>) => void;
  setLocalModelLimits: (model: string, limits: Partial<LocalModelLimits>) => void;
  setOutputTone: (outputTone: Preferences['outputTone']) => void;
  setResume: (resume: Preferences['resume']) => void;
  setApplicant: (applicant: Preferences['applicant']) => void;
  setPerformanceMode: (performanceMode: Preferences['performanceMode']) => void;
  setPromptQuality: (promptQuality: PromptQuality) => void;
  setDebugMode: (enabled: boolean) => void;
  setSemanticScoring: (enabled: boolean) => void;
  setOnboardingComplete: () => void;
  setContactPromptSeen: () => void;
  resetPreferences: () => void;
}

type PreferencesStore = Preferences & PreferencesActions;

export const usePreferencesStore = create<PreferencesStore>()(
  persist(
    (set) => ({
      ...defaultPreferences,
      setUserName: (userName: string) =>
        set((state) => ({
          ...state,
          userName,
          lastUpdated: new Date().toISOString(),
        })),

      setLanguage: (language: string) =>
        set((state) => ({
          ...state,
          language,
          lastUpdated: new Date().toISOString(),
        })),

      setAIModel: (aiModel: Preferences['aiModel']) =>
        set((state) => ({
          ...state,
          aiModel,
          lastUpdated: new Date().toISOString(),
        })),

      setAiProviderConfig: (aiProviderConfig: Preferences['aiProviderConfig']) =>
        set((state) => ({
          ...state,
          aiProviderConfig,
          lastUpdated: new Date().toISOString(),
        })),

      setActiveProvider: (provider: AiProvider) =>
        set((state) => ({
          ...state,
          aiProviderConfig: {
            activeProvider: provider,
            providers: state.aiProviderConfig?.providers ?? {},
          },
          lastUpdated: new Date().toISOString(),
        })),

      setProviderSettings: (provider: AiProvider, settings: Partial<PerProviderSettings>) =>
        set((state) => {
          const existing = state.aiProviderConfig?.providers?.[provider] ?? { model: '' };
          return {
            ...state,
            aiProviderConfig: {
              activeProvider: state.aiProviderConfig?.activeProvider ?? 'ollama',
              providers: {
                ...state.aiProviderConfig?.providers,
                [provider]: { ...existing, ...settings },
              },
            },
            lastUpdated: new Date().toISOString(),
          };
        }),

      // Per-model limits live under the local (ollama) provider, keyed by model
      // name, and are deep-merged so context-window and max-output update
      // independently.
      setLocalModelLimits: (model: string, limits: Partial<LocalModelLimits>) =>
        set((state) => {
          const ollama = state.aiProviderConfig?.providers?.ollama ?? { model: '' };
          const existingLimits = ollama.modelLimits ?? {};
          return {
            ...state,
            aiProviderConfig: {
              activeProvider: state.aiProviderConfig?.activeProvider ?? 'ollama',
              providers: {
                ...state.aiProviderConfig?.providers,
                ollama: {
                  ...ollama,
                  modelLimits: {
                    ...existingLimits,
                    [model]: { ...existingLimits[model], ...limits },
                  },
                },
              },
            },
            lastUpdated: new Date().toISOString(),
          };
        }),

      setOutputTone: (outputTone: Preferences['outputTone']) =>
        set((state) => ({
          ...state,
          outputTone,
          lastUpdated: new Date().toISOString(),
        })),

      setResume: (resume: Preferences['resume']) =>
        set((state) => ({
          ...state,
          resume,
          lastUpdated: new Date().toISOString(),
        })),

      setApplicant: (applicant: Preferences['applicant']) =>
        set((state) => ({
          ...state,
          applicant,
          lastUpdated: new Date().toISOString(),
        })),

      setPerformanceMode: (performanceMode: Preferences['performanceMode']) =>
        set((state) => ({
          ...state,
          performanceMode,
          lastUpdated: new Date().toISOString(),
        })),

      setPromptQuality: (promptQuality: PromptQuality) =>
        set((state) => ({
          ...state,
          promptQuality,
          lastUpdated: new Date().toISOString(),
        })),

      setDebugMode: (debugMode: boolean) =>
        set((state) => ({
          ...state,
          debugMode,
          lastUpdated: new Date().toISOString(),
        })),

      setSemanticScoring: (semanticScoring: boolean) =>
        set((state) => ({
          ...state,
          semanticScoring,
          lastUpdated: new Date().toISOString(),
        })),

      setOnboardingComplete: () =>
        set((state) => ({
          ...state,
          onboardingCompleted: true,
          lastUpdated: new Date().toISOString(),
        })),

      setContactPromptSeen: () =>
        set((state) => ({
          ...state,
          contactPromptSeen: true,
          lastUpdated: new Date().toISOString(),
        })),

      resetPreferences: () => set(defaultPreferences),
    }),
    {
      name: 'ai-job-hunter-preferences',
      storage: createJSONStorage(() => localStorage),
      version: STORE_VERSION,
      migrate: (persistedState: unknown, version: number) => {
        return migratePreferences(persistedState as Record<string, unknown>, version);
      },
    }
  )
);

// Selectors for common use cases
export const useUserName = () => usePreferencesStore((state) => state.userName);
export const useLanguage = () => usePreferencesStore((state) => state.language);
export const useAIModel = () => usePreferencesStore((state) => state.aiModel);
export const useAiProviderConfig = () => usePreferencesStore((state) => state.aiProviderConfig);
export const useOutputTone = () => usePreferencesStore((state) => state.outputTone);
export const useOnboardingCompleted = () =>
  usePreferencesStore((state) => state.onboardingCompleted);
export const useContactPromptSeen = () =>
  usePreferencesStore((state) => state.contactPromptSeen ?? false);
export const useResume = () => usePreferencesStore((state) => state.resume);
export const useApplicant = () => usePreferencesStore((state) => state.applicant);
export const usePerformanceMode = () => usePreferencesStore((state) => state.performanceMode);
export const usePromptQuality = () => usePreferencesStore((state) => state.promptQuality ?? 'auto');
export const useDebugMode = () => usePreferencesStore((state) => state.debugMode ?? false);
export const useSemanticScoring = () =>
  usePreferencesStore((state) => state.semanticScoring ?? false);
