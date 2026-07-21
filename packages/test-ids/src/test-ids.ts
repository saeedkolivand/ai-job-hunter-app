/**
 * Centralized, feature-namespaced test-id constants.
 *
 * Shape mirrors the translation-key namespace tree:
 *   TEST_IDS.<feature>.<name>  ←→  t('<feature>.<name>')
 *
 * String VALUES are byte-identical to the original inline strings — only the
 * reference site changes from a literal to a constant.
 *
 * Rule: pure string constants only — no React, no Node, no test-framework imports.
 */
export const TEST_IDS = {
  /** Chrome / cross-route layout stubs */
  layout: {
    pageShell: 'page-shell',
    pageHeader: 'page-header',
    pageTransition: 'page-transition',
    notFound: 'notfound',
    dashboard: 'dashboard',
  },

  /** Jobs feature — scraping, results, form */
  jobs: {
    scrapeForm: 'scrape-form',
    /** Bounded, scrollable wrapper around the page header + scrape form. */
    scrapeFormScroll: 'scrape-form-scroll',
    /** stub-only: no matching attribute on the real component */
    scrapeFilters: 'scrape-filters',
    aggregatorKeyHint: 'aggregator-key-hint',
    scrapeStartButton: 'scrape-start-button',
    jobsResults: 'jobs-results',
    jobsList: 'jobs-list',
    postingRow: 'posting-row',
    /** Cross-board cluster (ADR-029): one chip per non-self member source. */
    clusterSourceChip: 'cluster-source-chip',
    /** "All sources" section in the detail pane listing every cluster member. */
    clusterMembers: 'cluster-members',
    /** "Not a duplicate" split action on a non-canonical cluster member. */
    clusterSplitButton: 'cluster-split-button',
    /** Muted chip marking a recruiting/staffing-agency posting. */
    agencyChip: 'agency-chip',
    /** Filter toggle that hides agency postings from the list. */
    hideAgencyToggle: 'hide-agency-toggle',
  },

  /** Settings feature */
  settings: {
    defaultAccentDot: 'default-accent-dot',
    generalSection: 'general-section',
    appearanceCard: 'appearance-card',
    contactTab: 'contact-tab',
    aiTab: 'ai-tab',
    tonePrefs: 'tone-prefs',
    jobLocation: 'job-location',
    techStack: 'tech-stack',
    aggregator: 'aggregator',
    resumePrefs: 'resume-prefs',
    accountsTab: 'accounts-tab',
    privacyTab: 'privacy-tab',
    perfPrefs: 'perf-prefs',
    devPrefs: 'dev-prefs',
    aboutTab: 'about-tab',
  },

  /** Autopilot feature */
  autopilot: {
    card: 'autopilot-card',
    creationWizard: 'creation-wizard',
    emptyState: 'autopilot-empty-state',
    /** StepSchedule probe — used in wizard step test */
    probe: 'probe',
  },

  /** Applications (tracker) feature */
  applications: {
    list: 'applications-list',
    row: 'application-row',
    trackJobModal: 'track-job-modal',
  },

  /**
   * Documents feature — TailorFlow, DocumentsPage, GenerationOutput,
   * GeneratingPanel, ReferralModal, ai-generate OutputPanelDone.
   */
  documents: {
    generationCard: 'generation-card',
    interactionRow: 'interaction-row',
    /** stub-only: no matching attribute on the real component */
    tailorFlowStub: 'tailor-flow-stub',
    /** stub-only: no matching attribute on the real component — used in ApplicationDetailPage tests */
    tailorFlow: 'tailor-flow',
    tailorWizard: 'tailor-wizard',
    wizardNext: 'wizard-next',
    wizardGenerate: 'wizard-generate',
    generatingPanel: 'generating-panel',
    resultsPanel: 'results-panel',
    questionsModal: 'questions-modal',
    interviewModal: 'interview-modal',
    referralModal: 'referral-modal',
    modalShell: 'modal-shell',
    editableOutput: 'editable-output',
    editableInput: 'editable-input',
    saveBtn: 'save-btn',
    previewSlot: 'preview-slot',
    templatePicker: 'template-picker',
    pdfPreview: 'pdf-preview',
    thinkingBubble: 'thinking-bubble',
    stepDots: 'step-dots',
    jobAdViewTextarea: 'job-ad-view-textarea',
  },

  /** Resume shared components (ResumeInputCard) */
  resume: {
    review: 'review',
    uploadZone: 'upload-zone',
  },

  /** Shared generation component (EditableOutput, AccentPicker) */
  generation: {
    richTextEditor: 'rich-text-editor',
    rteSelectTrigger: 'rte-select-trigger',
    rteValue: 'rte-value',
    rteDeselectTrigger: 'rte-deselect-trigger',
    customPreview: 'custom-preview',
    pendingCommit: 'pending-commit',
    /** AccentPicker: "Template default" chip (clears the accent override). */
    accentDefault: 'accent-default',
    /** AccentPicker: a curated swatch — suffix with the accent id (`accent-swatch-navy`). */
    accentSwatch: 'accent-swatch',
    /** AccentPicker: the custom 6-hex input. */
    accentCustom: 'accent-custom',
    /** LetterLayoutPicker: one layout option — suffix with the layout id
     *  (`letter-layout-option-refined`). */
    letterLayoutOption: 'letter-layout-option',
  },

  /** Onboarding wizard */
  onboarding: {
    stepWelcome: 'step-welcome',
    stepResume: 'step-resume',
    stepAi: 'step-ai',
    stepResearch: 'step-research',
    stepBrowser: 'step-browser',
    stepAdzunaKey: 'step-adzuna-key',
    stepExtension: 'step-extension',
    stepAppearance: 'step-appearance',
    tour: 'tour',
  },
} as const;
