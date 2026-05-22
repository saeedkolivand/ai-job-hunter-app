import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

import type { AiProvider, PerProviderSettings, Preferences } from './preferences-schema';

// Migration function to handle version updates
const STORE_VERSION = 2;

const migratePreferences = (state: Record<string, unknown>, version: number): Preferences => {
  // v0 → v1: baseline
  if (version < 1) {
    state = { ...state, version: 1, lastUpdated: new Date().toISOString() };
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
  remote: 'any',
  techStack: [],
  seniority: 'any',
  performanceMode: 'balanced',
  onboardingCompleted: false,
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
  setOutputTone: (outputTone: Preferences['outputTone']) => void;
  setLocation: (location: Preferences['location']) => void;
  setRemote: (remote: Preferences['remote']) => void;
  setTechStack: (techStack: Preferences['techStack']) => void;
  addTechStackItem: (item: Preferences['techStack'][number]) => void;
  removeTechStackItem: (name: string) => void;
  setSeniority: (seniority: Preferences['seniority']) => void;
  setSalary: (salary: Preferences['salary']) => void;
  setResume: (resume: Preferences['resume']) => void;
  setPerformanceMode: (performanceMode: Preferences['performanceMode']) => void;
  setOnboardingComplete: () => void;
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

      setOutputTone: (outputTone: Preferences['outputTone']) =>
        set((state) => ({
          ...state,
          outputTone,
          lastUpdated: new Date().toISOString(),
        })),

      setLocation: (location: Preferences['location']) =>
        set((state) => ({
          ...state,
          location,
          lastUpdated: new Date().toISOString(),
        })),

      setRemote: (remote: Preferences['remote']) =>
        set((state) => ({
          ...state,
          remote,
          lastUpdated: new Date().toISOString(),
        })),

      setTechStack: (techStack: Preferences['techStack']) =>
        set((state) => ({
          ...state,
          techStack,
          lastUpdated: new Date().toISOString(),
        })),

      addTechStackItem: (item: Preferences['techStack'][number]) =>
        set((state) => ({
          ...state,
          techStack: [...state.techStack, item],
          lastUpdated: new Date().toISOString(),
        })),

      removeTechStackItem: (name: string) =>
        set((state) => ({
          ...state,
          techStack: state.techStack.filter((item) => item.name !== name),
          lastUpdated: new Date().toISOString(),
        })),

      setSeniority: (seniority: Preferences['seniority']) =>
        set((state) => ({
          ...state,
          seniority,
          lastUpdated: new Date().toISOString(),
        })),

      setSalary: (salary: Preferences['salary']) =>
        set((state) => ({
          ...state,
          salary,
          lastUpdated: new Date().toISOString(),
        })),

      setResume: (resume: Preferences['resume']) =>
        set((state) => ({
          ...state,
          resume,
          lastUpdated: new Date().toISOString(),
        })),

      setPerformanceMode: (performanceMode: Preferences['performanceMode']) =>
        set((state) => ({
          ...state,
          performanceMode,
          lastUpdated: new Date().toISOString(),
        })),

      setOnboardingComplete: () =>
        set((state) => ({
          ...state,
          onboardingCompleted: true,
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
export const useLocation = () => usePreferencesStore((state) => state.location);
export const useRemote = () => usePreferencesStore((state) => state.remote);
export const useTechStack = () => usePreferencesStore((state) => state.techStack);
export const useSeniority = () => usePreferencesStore((state) => state.seniority);
export const useOnboardingCompleted = () =>
  usePreferencesStore((state) => state.onboardingCompleted);
export const useSalary = () => usePreferencesStore((state) => state.salary);
export const useResume = () => usePreferencesStore((state) => state.resume);
export const usePerformanceMode = () => usePreferencesStore((state) => state.performanceMode);
