import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import type { Preferences } from './preferences-schema';

// Migration function to handle version updates
const STORE_VERSION = 1;

const migratePreferences = (state: Record<string, unknown>): Preferences => {
  const version = (state.version as number | undefined) || 0;

  // Migration from version 0 to 1
  if (version === 0) {
    return { ...state, version: 1, lastUpdated: new Date().toISOString() } as Preferences;
  }

  // Future migrations can be added here
  if (version < STORE_VERSION) {
    return {
      ...state,
      version: STORE_VERSION,
      lastUpdated: new Date().toISOString(),
    } as Preferences;
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
      version: 1,
      migrate: (persistedState: unknown, version: number) => {
        const state = persistedState as Record<string, unknown>;
        if (version === 0) {
          return migratePreferences(state);
        }
        return state as Preferences;
      },
    }
  )
);

// Selectors for common use cases
export const useUserName = () => usePreferencesStore((state) => state.userName);
export const useLanguage = () => usePreferencesStore((state) => state.language);
export const useAIModel = () => usePreferencesStore((state) => state.aiModel);
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
