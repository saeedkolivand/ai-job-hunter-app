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
    /** Scrollable body of the scrape drawer — owns the form's overflow so tall
     *  form content can never push the Start button out of reach. */
    scrapeFormScroll: 'scrape-form-scroll',
    /** Compact command bar above the results (title + filters + view + actions). */
    commandBar: 'jobs-command-bar',
    /** Second command-bar row: the active-filter chips (only when non-empty). */
    filterChips: 'jobs-filter-chips',
    /** Live scrape strip in the command bar — progress label + cancel. */
    scrapeStatusStrip: 'jobs-scrape-status',
    /** Always-mounted sr-only live region the command bar writes status into. */
    scrapeStatusLive: 'jobs-scrape-status-live',
    /** stub-only: no matching attribute on the real component */
    scrapeFilters: 'scrape-filters',
    /** The manual search form's remote/hybrid/on-site multi-select group. */
    workTypeFilter: 'work-type-filter',
    /** Commits the filter-box text as a ranked hybrid search (Enter also fires it). */
    searchButton: 'jobs-search-button',
    /** "Ranked by …" banner above ranked results — surfaces which arms ran and,
     *  when semantic ranking is off, the one-click enable action. */
    searchBanner: 'jobs-search-banner',
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
    /** ATS slug typeahead (ADR-030): the text input users type company slugs into. */
    companyTypeahead: 'company-typeahead',
    /** One suggestion row in the slug typeahead. */
    companySuggestion: 'company-suggestion',
    /** Per-row star (watch) toggle in the slug typeahead. */
    companyStarToggle: 'company-star-toggle',
    /** A selected-company chip feeding the scrape `companies` array. */
    companyChip: 'company-chip',
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
    /** Watched-companies-only target toggle (ADR-030) in the wizard board step. */
    watchedCompaniesToggle: 'watched-companies-toggle',
  },

  /** Applications (tracker) feature */
  applications: {
    list: 'applications-list',
    row: 'application-row',
    trackJobModal: 'track-job-modal',
    /** One pipeline-strip stat card; `data-group` carries the stage-group id. */
    pipelineCard: 'pipeline-card',
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
    /** Inline banner surfacing a failed generation's reason on the wizard. */
    generationError: 'generation-error',
    /** Inline banner acknowledging a cancel that produced no output. */
    generationCancelled: 'generation-cancelled',
    /** The persistently-mounted sr-only live region (CR-7) — distinct from
     *  the visual banners' own `role="status"`, which also exist. */
    liveAnnouncer: 'live-announcer',
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
    /** GenerationOutput's scrolling document region — carries the min-height floor. */
    documentRegion: 'document-region',
    /** JobAdView's Score tab — root panel + each metric's value cell. */
    jobAdViewScorePanel: 'job-ad-view-score-panel',
    jobAdViewScoreMatch: 'job-ad-view-score-match',
    jobAdViewScoreCoverage: 'job-ad-view-score-coverage',
    jobAdViewScoreSemantic: 'job-ad-view-score-semantic',
    /** Résumé result's compact score strip (GenerationOutput) — root + the
     *  coverage metric's value cell. Shares its `MatchScore` predicates/row
     *  with the Score tab above (see `MatchScoreMetric.tsx`) but is its own
     *  surface, so it gets its own ids rather than reusing the tab's. */
    scoreStrip: 'score-strip',
    scoreStripCoverage: 'score-strip-coverage',
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
    stepAutoIndex: 'step-auto-index',
    stepCrashReporting: 'step-crash-reporting',
    stepAppearance: 'step-appearance',
    tour: 'tour',
  },

  /** Dashboard — the persistent next-step row above the quick actions */
  dashboard: {
    /** Wrapper around the ActionTile nudging the first unmet setup step. */
    nextStepTile: 'dashboard-next-step-tile',
    /** The slim "setup complete" row that replaces the tile once every step is met. */
    nextStepDone: 'dashboard-next-step-done',
    /** The same slim row, neutral copy, when a signal query failed and the tile
     *  cannot say which step is next — distinct from `nextStepDone` so a test
     *  can prove the "setup complete" claim is NOT what a failed read renders. */
    nextStepUnavailable: 'dashboard-next-step-unavailable',
  },

  /** Help & Support — the searchable help page (ADR-041) */
  support: {
    /** Search box above the sections; filters entries with a word-AND substring match. */
    searchInput: 'support-search-input',
    /** Wrapper around the EmptyState shown when no entry in any section matches. */
    emptyState: 'support-empty-state',

    // ── Help chat (ADR-043) — the grounded assistant above the search box ──
    /**
     * The whole chat card. Always rendered on the Help page — when AI is not
     * usable the card stays and shows `AiSetupHint` inside it, so its absence
     * means the page itself failed to render.
     */
    chatCard: 'support-chat-card',
    /** The question box. */
    chatInput: 'support-chat-input',
    /** Ask — submits the typed question. */
    chatAsk: 'support-chat-ask',
    /** Stop — aborts the in-flight stream, keeping the partial answer. */
    chatStop: 'support-chat-stop',
    /** The live/streamed answer region for the current question. */
    chatAnswer: 'support-chat-answer',
    /** One rendered turn of the transcript (user or assistant). */
    chatTurn: 'support-chat-turn',
    /** A "Based on" chip; clicking it searches the page for that entry's title. */
    chatSource: 'support-chat-source',
    /** Notice shown when the user's semantic-scoring opt-in is OFF (`dense: 'skipped'`). */
    chatKeywordNotice: 'support-chat-keyword-notice',
    /**
     * Notice shown when semantic scoring is ON but the embedding failed
     * (`dense: 'unavailable'`) — nothing for the user to switch, so it carries
     * no Settings link.
     */
    chatDenseNotice: 'support-chat-dense-notice',
    /** Error row shown when retrieval or generation failed. */
    chatError: 'support-chat-error',
    /** Re-sends the failed question from the error row. */
    chatRetry: 'support-chat-retry',
  },
} as const;
