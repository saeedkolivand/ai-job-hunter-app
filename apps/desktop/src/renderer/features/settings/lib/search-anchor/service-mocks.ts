// Factory bodies for the global `vi.mock` stubs in `../search-anchor.test.tsx`: every
// service/IPC hook, store and UI helper the rendered sections touch, so no
// QueryClient or Tauri context is needed. The `vi.mock` calls themselves stay in the
// test file (they only hoist there) and load these lazily.
import { vi } from 'vitest';

export const translationsMock = { useTranslation: () => ({ t: (k: string) => k }) };

// Stub services consumed by the section components we render.
export const servicesMock: Record<string, unknown> = {
  // GeneralSection / UpdateSection
  useLaunchAtLogin: () => ({ data: false }),
  useSetLaunchAtLogin: () => ({ mutate: vi.fn(), isPending: false }),
  useSetCloseToTray: () => ({ mutate: vi.fn(), isPending: false }),
  useAppVersion: () => ({ data: '1.0.0' }),
  useOpenExternal: () => ({ mutate: vi.fn(), isPending: false }),
  useUpdater: () => ({
    status: { state: 'idle' },
    check: vi.fn(),
    download: vi.fn(),
    install: vi.fn(),
  }),
  useChangelog: () => ({ data: undefined, isPending: false }),
  // AppearanceCard
  useSystemAccent: () => ({ data: { supported: false } }),
  // ContactProfileTab / ApplicantDetailsSection
  useContactProfile: () => ({ data: undefined }),
  useSetContactProfile: () => ({ mutate: vi.fn(), isPending: false }),
  useJobPreferences: () => ({ data: undefined }),
  useSetJobPreferences: () => ({ mutate: vi.fn(), isPending: false }),
  useSetExtraAgencyCompanies: () => ({ mutate: vi.fn(), isPending: false }),
  // AccountsSettingsTab
  useCredentialsAvailable: () => ({ data: true }),
  useBoardSession: () => ({ data: undefined }),
  useLoginBoard: () => ({ mutate: vi.fn(), isPending: false }),
  useLogoutBoard: () => ({ mutate: vi.fn(), isPending: false }),
  // ExtensionBridgeSection
  useExtensionBridgeStatus: () => ({ data: undefined }),
  useGeneratePairingToken: () => ({ mutate: vi.fn(), isPending: false }),
  // PrivacySettingsTab
  useCrashReporting: () => ({ data: { enabled: true, consentShown: true }, isLoading: false }),
  useSetCrashReporting: () => ({ mutate: vi.fn(), isPending: false }),
  useClearInteractions: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useExportData: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useImportData: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useResetApp: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useSignOutAll: () => ({ mutateAsync: vi.fn(), isPending: false }),
  // AISettingsTab via useProviderKeys
  useActiveProvider: () => ({ data: undefined }),
  useSetActiveProvider: () => ({ mutate: vi.fn(), isPending: false }),
  useConnectedProviders: () => ({ data: [] }),
  useProviderKeyStatus: () => ({ data: {} }),
  useProviderConfig: () => ({ data: undefined }),
  useOllamaModels: () => ({ data: undefined, isLoading: false }),
  useSetProviderKey: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRemoveProviderKey: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useTestProviderKey: () => ({ mutateAsync: vi.fn(), isPending: false }),
  usePullOllamaModel: () => ({ mutateAsync: vi.fn(), isPending: false }),
  // EmbeddingsSettings
  useEmbeddingStatus: () => ({ data: undefined, refetch: vi.fn() }),
  useSetEmbeddingConfig: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useReembedAll: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useJobEvents: (_handler: unknown) => undefined,
  // CompanyResearchSettings
  useCompanyResearchConfig: () => ({ data: undefined }),
  useSetCompanyResearchConfig: () => ({ mutate: vi.fn(), isPending: false }),
  // DeveloperPreferences
  useOpenDevtools: () => ({ mutate: vi.fn(), isPending: false }),
  useExportDiagnostics: () => ({ mutateAsync: vi.fn(), isPending: false }),
  // AgentCliSection
  useAgentCliInfo: () => ({ data: { exePath: '/tmp/ajh-tauri' }, isPending: false }),
  // ResumePreferences — covered via SettingsContent wrappers
  useDocuments: () => ({ data: [], isLoading: false }),
  useRemoveDocument: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useSetDefaultDocument: () => ({ mutateAsync: vi.fn(), isPending: false }),
  // AggregatorKeysSettings
  useHasProviderKey: () => ({ data: { has: false } }),
  useScrapingSettings: () => ({
    data: { apifyLinkedinEnabled: false, apifyLinkedinActorId: undefined },
  }),
  useUpdateScrapingSettings: () => ({ mutateAsync: vi.fn(), isPending: false }),
  // Window controls (service hook)
  useWindowControls: () => ({
    resetPosition: vi.fn(),
    hideApp: vi.fn(),
    isMacos: false,
  }),
};

// Stub the Zustand stores that section components subscribe to.
export const preferencesStoreMock: Record<string, unknown> = {
  useCloseToTray: () => false,
  useOnboardingCompleted: () => true,
  usePreferencesStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      resetOnboarding: vi.fn(),
      addRecentLocation: vi.fn(),
      setDebugMode: vi.fn(),
      setOutputTone: vi.fn(),
      setPerformanceMode: vi.fn(),
      setCustomPerformance: vi.fn(),
      setFetchCompanyLogos: vi.fn(),
    }),
  useFetchCompanyLogos: () => false,
  useDebugMode: () => false,
  useOutputTone: () => 'professional',
  usePerformanceMode: () => 'balanced',
  useResolvedPerformanceProfile: () => ({
    visual: { aurora: true, nebula: true, cursorGlow: true, animations: true, blur: 'full' },
    backend: { concurrency: 'balanced', keepAlive: 'balanced', cache: 'balanced' },
  }),
  // JobLocationPreferences reads this from the store
  useRecentLocations: () => [],
};

// Stub applyThemeAnimated so AppearanceCard has no localStorage side-effect.
export async function uiMock(
  importOriginal: <T>() => Promise<T>
): Promise<Record<string, unknown>> {
  const actual = await importOriginal<Record<string, unknown>>();
  return {
    ...actual,
    applyThemeAnimated: vi.fn(),
    useNotification: () => ({
      open: vi.fn(),
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
      destroy: vi.fn(),
    }),
  };
}
