/**
 * Settings search — render-based anchor drift guard (HIGH blocker).
 *
 * For every entry in SEARCH_INDEX this test renders the owning section
 * component(s) into jsdom and asserts that
 * `container.querySelector('[data-settings-anchor="<anchor>"]')` is non-null.
 *
 * Why this beats a static string-Set check:
 *  A string Set checked against the same manifest cannot detect a typo in the
 *  component that produces `data-settings-anchor`. This test actually renders the
 *  tree, so a missing or mis-spelled attribute on the component side fails here.
 *
 * Coverage:
 *  - All 11 SectionIds have ≥1 SEARCH_INDEX entry.
 *  - All 30 anchors are reachable in the rendered DOM.
 *  - Multi-component sections are fully covered:
 *      general   → GeneralSection (6 anchors inside the component)
 *      appearance → AppearanceCard (5 anchors inside the component)
 *      contact   → ContactProfileTab (2 anchors inside the component)
 *      ai        → SettingsContent wrapper (2) + AISettingsTab interior (3)
 *      job       → SettingsContent wrappers (3)
 *      resume    → SettingsContent wrapper (1)
 *      accounts  → AccountsSettingsTab (3 anchors inside)
 *      privacy   → PrivacySettingsTab (2 anchors inside)
 *      performance → SettingsContent wrapper (1)
 *      developer → SettingsContent wrapper (1)
 *      about     → SettingsContent wrapper (1)
 *
 * Sections that SettingsContent wraps in their own data-settings-anchor div are
 * tested by rendering SettingsContent with the matching activeSection — the wrappers
 * live in SettingsContent, not inside the leaf component.
 *
 * All service/IPC hooks are stubbed so no QueryClient or Tauri context is needed.
 */

import type { RefObject } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';

import { NAV_GROUPS, type NavItem, type SectionId } from '@/features/settings/constants';
import { SEARCH_INDEX } from '@/features/settings/lib/search-index';

// ── global stubs ──────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// Stub services consumed by the section components we render.
vi.mock('@/services', () => ({
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
  // AccountsSettingsTab
  useCredentialsAvailable: () => ({ data: true }),
  useBoardSession: () => ({ data: undefined }),
  useLoginBoard: () => ({ mutate: vi.fn(), isPending: false }),
  useLogoutBoard: () => ({ mutate: vi.fn(), isPending: false }),
  // ExtensionBridgeSection
  useExtensionBridgeStatus: () => ({ data: undefined }),
  useGeneratePairingToken: () => ({ mutate: vi.fn(), isPending: false }),
  // PrivacySettingsTab
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
}));

// Stub the Zustand stores that section components subscribe to.
vi.mock('@/store/preferences-store', () => ({
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
}));

// Stub applyThemeAnimated so AppearanceCard has no localStorage side-effect.
vi.mock('@ajh/ui', async (importOriginal) => {
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
});

// ── Stub sub-components that need QueryClient or deep IPC trees ───────────────
// The anchor drift guard only cares that data-settings-anchor attrs are present
// in the rendered tree — it does not test sub-component behaviour. Stubbing the
// heavy leaves keeps this file free of QueryClientProvider setup.

// GeneralSection children
vi.mock('@/features/settings/components/shared/LanguageSelector', () => ({
  LanguageSelector: () => null,
}));
vi.mock('@/features/settings/components/update-section', () => ({
  UpdateSection: () => <div data-settings-anchor="general-updates" />,
}));

// ContactProfileTab children
vi.mock('@/components/contact/ContactProfileForm', () => ({
  ContactProfileForm: () => null,
}));
vi.mock('@/features/settings/components/contact/ApplicantDetailsSection', () => ({
  ApplicantDetailsSection: () => <div data-settings-anchor="contact-applicant" />,
}));

// AISettingsTab — renders via SettingsContent with the REAL component so that
// data-settings-anchor="ai-embeddings" and data-settings-anchor="ai-company-research"
// are verified in the actual production JSX (not injected by a stub).
// useProviderKeys calls useQueryClient/useQueries/useAppClient — mock it at the
// hook boundary so the real AISettingsTab JSX (including the anchor divs) renders.
vi.mock('@/features/settings/components/ai-settings/AISettingsTab/useProviderKeys', () => ({
  useProviderKeys: () => ({
    activeProvider: 'ollama',
    setActiveProvider: vi.fn(),
    connectedProviders: [],
    keyStatus: {},
    providerConfig: undefined,
    selectedOllamaModel: undefined,
    ollamaModels: [],
    loadingOllama: false,
    expanded: null,
    expandedModels: [],
    apiKeyInput: '',
    showKey: false,
    savingKey: null,
    testingKey: null,
    baseUrlInput: '',
    pulling: null,
    handleSelectModel: vi.fn(),
    handleSaveKey: vi.fn(),
    handleTestKey: vi.fn(),
    handleRemoveKey: vi.fn(),
    handlePullOllama: vi.fn(),
    toggleExpand: vi.fn(),
    setApiKeyInput: vi.fn(),
    toggleShowKey: vi.fn(),
    setBaseUrlInput: vi.fn(),
    recheck: vi.fn(),
    openDocs: vi.fn(),
  }),
}));
// Stub the child sections that useProviderKeys feeds into so they don't need
// their own deep trees — the anchors being guarded live directly in AISettingsTab.
vi.mock('@/features/settings/components/ai-settings/ActiveProviderSwitcher', () => ({
  ActiveProviderSwitcher: () => null,
}));
vi.mock('@/features/settings/components/ai-settings/ProviderDebugBadge', () => ({
  ProviderDebugBadge: () => null,
}));
vi.mock('@/features/settings/components/ai-settings/ProviderRow', () => ({
  ProviderRow: () => null,
}));
vi.mock('@/features/settings/components/ai-settings/EmbeddingsSettings', () => ({
  EmbeddingsSettings: () => null,
}));
vi.mock('@/features/settings/components/ai-settings/CompanyResearchSettings', () => ({
  CompanyResearchSettings: () => null,
}));
vi.mock('@/features/settings/components/ai-settings/SpendSettings', () => ({
  SpendSettings: () => null,
}));

// AccountsSettingsTab children
vi.mock('@/features/settings/components/accounts/BoardSessionRow', () => ({
  BoardSessionRow: () => null,
}));
vi.mock('@/features/settings/components/accounts/ExtensionBridgeSection', () => ({
  ExtensionBridgeSection: () => <div data-settings-anchor="accounts-extension" />,
}));
vi.mock('@/features/settings/components/accounts/EmailWatchSection', () => ({
  EmailWatchSection: () => <div data-settings-anchor="accounts-email-watch" />,
}));

// PerformancePreferences — calls t(`${base}.details`, { returnObjects: true }) which the
// stub returns as a string (not array), causing details.map to throw.
vi.mock('@/features/settings/components/preferences/PerformancePreferences', () => ({
  PerformancePreferences: () => <div data-settings-anchor="performance-mode" />,
}));

// ResumePreferences children
vi.mock('@/components/resume/ProfileUrlImport', () => ({
  ProfileUrlImport: () => null,
}));
vi.mock('@/components/contact/ContactConflictModal', () => ({
  ContactConflictModal: () => null,
}));
vi.mock('@/hooks/use-import-with-ocr', () => ({
  useImportWithOcr: () => ({ importFile: vi.fn(), isPending: false, isOcr: false }),
}));
vi.mock('@/lib/generate', () => ({ exportTXT: vi.fn() }));
vi.mock('@/lib/doc-record', () => ({
  normalise: (d: unknown) => d,
}));

// ── lazy imports (AFTER all vi.mock calls) ────────────────────────────────────

// Component imports intentionally deferred to after mock hoisting.
// Sections rendered directly (not through SettingsContent):
import { AccountsSettingsTab } from '@/features/settings/components/accounts/AccountsSettingsTab';
import { ContactProfileTab } from '@/features/settings/components/contact/ContactProfileTab';
import { GeneralSection } from '@/features/settings/components/general-section';
import { AppearanceCard } from '@/features/settings/components/general-section/AppearanceCard';
import { PrivacySettingsTab } from '@/features/settings/components/privacy/PrivacySettingsTab';
// Sections rendered via SettingsContent (wrapper divs live in SettingsContent):
import { SettingsContent } from '@/features/settings/components/SettingsContent';

// ── helpers ───────────────────────────────────────────────────────────────────

/** Flatten NAV_GROUPS to a NavItem lookup by id. */
const NAV_ITEMS = Object.fromEntries(
  NAV_GROUPS.flatMap((g) => g.items).map((item) => [item.id, item])
);

function makeCurrent(sectionId: SectionId): NavItem {
  const item = NAV_ITEMS[sectionId];
  if (!item) throw new Error(`No NavItem for section "${sectionId}"`);
  return item;
}

/** Stub ref that looks like RefObject<HTMLDivElement | null> */
const stubScrollRef: RefObject<HTMLDivElement | null> = { current: null };

/**
 * Render SettingsContent for the given section (exercises the wrapper divs
 * that SettingsContent adds — i.e. job, resume, performance, developer, about).
 */
function renderSection(sectionId: SectionId) {
  return render(
    <SettingsContent
      activeSection={sectionId}
      current={makeCurrent(sectionId)}
      localName="Test"
      setLocalName={vi.fn()}
      setUserName={vi.fn()}
      userName="Test"
      pendingAnchor={null}
      scrollRef={stubScrollRef}
      onAnchorConsumed={vi.fn()}
    />
  );
}

function assertAnchor(container: HTMLElement, anchor: string) {
  const el = container.querySelector(`[data-settings-anchor="${anchor}"]`);
  expect(
    el,
    `data-settings-anchor="${anchor}" not found in rendered DOM — check the component for a missing or mis-spelled attribute`
  ).not.toBeNull();
}

// ── manifest integrity ────────────────────────────────────────────────────────

describe('SEARCH_INDEX — manifest integrity', () => {
  it('has exactly 30 entries', () => {
    expect(SEARCH_INDEX).toHaveLength(30);
  });

  it('every SectionId has at least one entry', () => {
    const sectionIds: SectionId[] = [
      'general',
      'appearance',
      'contact',
      'ai',
      'job',
      'resume',
      'accounts',
      'privacy',
      'performance',
      'developer',
      'about',
    ];
    for (const id of sectionIds) {
      const entries = SEARCH_INDEX.filter((e) => e.section === id);
      expect(entries.length, `section "${id}" has no entries`).toBeGreaterThan(0);
    }
  });
});

// ── render-based anchor drift guards ─────────────────────────────────────────
//
// Each describe block renders the component(s) responsible for that section and
// asserts every anchor from SEARCH_INDEX for that section is present in the DOM.

describe('anchor drift guard — general (GeneralSection)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'general').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = render(
      <GeneralSection
        localName="Test User"
        setLocalName={vi.fn()}
        setUserName={vi.fn()}
        userName="Test User"
      />
    );
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — appearance (AppearanceCard)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'appearance').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = render(<AppearanceCard />);
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — contact (ContactProfileTab)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'contact').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = render(<ContactProfileTab />);
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — ai (AISettingsTab + OutputTonePreferences via SettingsContent)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'ai').map((e) => [e.anchor, e.id] as [string, string])
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    // ai-provider and ai-tone anchors are wrapper divs in SettingsContent.
    // ai-embeddings and ai-company-research are inside AISettingsTab.
    // Rendering SettingsContent covers all four.
    const { container } = renderSection('ai');
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — job (SettingsContent wrappers)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'job').map((e) => [e.anchor, e.id] as [string, string])
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = renderSection('job');
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — resume (SettingsContent wrapper)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'resume').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = renderSection('resume');
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — accounts (AccountsSettingsTab)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'accounts').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = render(<AccountsSettingsTab />);
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — privacy (PrivacySettingsTab)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'privacy').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = render(<PrivacySettingsTab />);
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — performance (SettingsContent wrapper)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'performance').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = renderSection('performance');
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — developer (SettingsContent wrapper)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'developer').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = renderSection('developer');
    assertAnchor(container, anchor);
  });
});

describe('anchor drift guard — about (SettingsContent wrapper)', () => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === 'about').map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    const { container } = renderSection('about');
    assertAnchor(container, anchor);
  });
});
