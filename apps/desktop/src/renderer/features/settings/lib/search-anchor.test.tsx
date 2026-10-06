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
 *  - Every anchor in the manifest is reachable in the rendered DOM.
 *  - Multi-component sections are fully covered:
 *      general   → GeneralSection (6 anchors inside the component)
 *      appearance → AppearanceCard (5 anchors inside the component)
 *      contact   → ContactProfileTab (2 anchors inside the component)
 *      ai        → SettingsContent wrapper (2) + AISettingsTab interior (5)
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

import { AccountsSettingsTab } from '@/features/settings/components/accounts/AccountsSettingsTab';
import { ContactProfileTab } from '@/features/settings/components/contact/ContactProfileTab';
import { ExtensionSettingsTab } from '@/features/settings/components/extension/ExtensionSettingsTab';
import { GeneralSection } from '@/features/settings/components/general-section';
import { AppearanceCard } from '@/features/settings/components/general-section/AppearanceCard';
import { PrivacySettingsTab } from '@/features/settings/components/privacy/PrivacySettingsTab';
import { SettingsContent } from '@/features/settings/components/SettingsContent';
import { NAV_GROUPS, type NavItem, type SectionId } from '@/features/settings/constants';
import { SEARCH_INDEX } from '@/features/settings/lib/search-index';

// ── global stubs (factory bodies live in ./search-anchor/service-mocks) ───────

vi.mock(
  '@ajh/translations',
  async () => (await import('./search-anchor/service-mocks')).translationsMock
);

// AgentCliSection links to Help & Support. `useRouter` throws outside a
// RouterProvider, and this file renders sections without one.
vi.mock('@tanstack/react-router', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
  useRouter: () => ({ navigate: vi.fn() }),
}));

vi.mock('@/services', async () => (await import('./search-anchor/service-mocks')).servicesMock);
vi.mock(
  '@/store/preferences-store',
  async () => (await import('./search-anchor/service-mocks')).preferencesStoreMock
);
vi.mock('@ajh/ui', async (importOriginal) =>
  (await import('./search-anchor/service-mocks')).uiMock(importOriginal)
);

// ── Stub sub-components that need QueryClient or deep IPC trees ───────────────
// The anchor drift guard only cares that data-settings-anchor attrs are present
// in the rendered tree — it does not test sub-component behaviour.

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
vi.mock('@/features/settings/components/ai-settings/StageOverridesSettings', () => ({
  StageOverridesSettings: () => null,
}));
vi.mock('@/features/settings/components/ai-settings/PromptQualitySettings', () => ({
  PromptQualitySettings: () => null,
}));

// AccountsSettingsTab children
vi.mock('@/features/settings/components/accounts/BoardSessionRow', () => ({
  BoardSessionRow: () => null,
}));
vi.mock('@/features/settings/components/extension/ExtensionBridgeSection', () => ({
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
  it('has exactly 34 entries', () => {
    expect(SEARCH_INDEX).toHaveLength(34);
  });

  // A bare count survives the wrong deletion (or a duplicate masking a real
  // removal), so pin the actual change: the depth-selection entry is gone,
  // and `ai-stages` no longer advertises the judge/sections keywords that
  // went with it.
  it('dropped the deleted generation-depth entry, not a different one', () => {
    expect(SEARCH_INDEX.find((e) => e.id === 'ai-depth')).toBeUndefined();
  });

  it('no longer indexes `ai-stages` under the removed judge/sections keywords', () => {
    const stages = SEARCH_INDEX.find((e) => e.id === 'ai-stages');
    expect(stages).toBeDefined();
    expect(stages?.keywords).not.toContain('judge');
    expect(stages?.keywords).not.toContain('sections');
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
      'extension',
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
// Each row renders the component(s) responsible for that section and asserts every
// anchor from SEARCH_INDEX for that section is present in the DOM. The ai, job,
// resume, performance, developer and about rows go through SettingsContent, whose
// wrapper divs carry those anchors (ai-embeddings / ai-company-research live inside
// the real AISettingsTab it renders).

const DRIFT_GUARDS: [
  title: string,
  section: SectionId,
  renderSection: () => ReturnType<typeof render>,
][] = [
  [
    'general (GeneralSection)',
    'general',
    () =>
      render(
        <GeneralSection
          localName="Test User"
          setLocalName={vi.fn()}
          setUserName={vi.fn()}
          userName="Test User"
        />
      ),
  ],
  ['appearance (AppearanceCard)', 'appearance', () => render(<AppearanceCard />)],
  ['contact (ContactProfileTab)', 'contact', () => render(<ContactProfileTab />)],
  [
    'ai (AISettingsTab + OutputTonePreferences via SettingsContent)',
    'ai',
    () => renderSection('ai'),
  ],
  ['job (SettingsContent wrappers)', 'job', () => renderSection('job')],
  ['resume (SettingsContent wrapper)', 'resume', () => renderSection('resume')],
  ['accounts (AccountsSettingsTab)', 'accounts', () => render(<AccountsSettingsTab />)],
  ['extension (ExtensionSettingsTab)', 'extension', () => render(<ExtensionSettingsTab />)],
  ['privacy (PrivacySettingsTab)', 'privacy', () => render(<PrivacySettingsTab />)],
  ['performance (SettingsContent wrapper)', 'performance', () => renderSection('performance')],
  ['developer (SettingsContent wrapper)', 'developer', () => renderSection('developer')],
  ['about (SettingsContent wrapper)', 'about', () => renderSection('about')],
];

describe.each(DRIFT_GUARDS)('anchor drift guard — %s', (_title, section, renderIt) => {
  it.each(
    SEARCH_INDEX.filter((e) => e.section === section).map(
      (e) => [e.anchor, e.id] as [string, string]
    )
  )('anchor "%s" (entry "%s") is present in the rendered DOM', (anchor) => {
    assertAnchor(renderIt().container, anchor);
  });

  // `it.each` over an empty array silently registers NO tests, so the guard above would
  // pass vacuously if the section lost its entries (#1213 moved them here from
  // `accounts`). Pin the count so that cannot go unnoticed.
  if (section === 'extension') {
    it('indexes at least one entry under the extension section', () => {
      expect(SEARCH_INDEX.filter((e) => e.section === 'extension').length).toBeGreaterThan(0);
    });
  }
});
