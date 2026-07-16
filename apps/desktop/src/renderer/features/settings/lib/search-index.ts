import type { SectionId } from '@/features/settings/constants';

/**
 * One searchable entry per setting control/card.
 *
 * - `titleKey`  — existing i18n key for the label displayed in results
 * - `keywords`  — locale-invariant synonyms searched verbatim
 * - `anchor`    — stable `data-settings-anchor` value on the rendered element
 */
export interface SearchEntry {
  id: string;
  section: SectionId;
  titleKey: string;
  keywords: string[];
  anchor: string;
}

export const SEARCH_INDEX: SearchEntry[] = [
  // ── general ─────────────────────────────────────────────────────────────────
  {
    id: 'general-profile',
    section: 'general',
    titleKey: 'settings.profile.title',
    keywords: ['name', 'display', 'user', 'profile'],
    anchor: 'general-profile',
  },
  {
    id: 'general-language',
    section: 'general',
    titleKey: 'settings.language.title',
    keywords: ['language', 'locale', 'english', 'german', 'deutsch', 'sprache', 'i18n'],
    anchor: 'general-language',
  },
  {
    id: 'general-onboarding',
    section: 'general',
    titleKey: 'settings.onboarding.title',
    keywords: ['onboarding', 'wizard', 'tour', 'replay', 'intro', 'welcome'],
    anchor: 'general-onboarding',
  },
  {
    id: 'general-startup',
    section: 'general',
    titleKey: 'settings.startup.title',
    keywords: ['startup', 'launch', 'login', 'tray', 'boot', 'autostart', 'close'],
    anchor: 'general-startup',
  },
  {
    id: 'general-window',
    section: 'general',
    titleKey: 'settings.window.title',
    keywords: ['window', 'position', 'reset', 'monitor', 'hide', 'move'],
    anchor: 'general-window',
  },
  {
    id: 'general-updates',
    section: 'general',
    titleKey: 'settings.update.title',
    keywords: ['update', 'version', 'upgrade', 'release', 'changelog', 'install'],
    anchor: 'general-updates',
  },

  // ── appearance ───────────────────────────────────────────────────────────────
  {
    id: 'appearance-theme',
    section: 'appearance',
    titleKey: 'settings.appearance.scheme',
    keywords: ['theme', 'dark', 'light', 'system', 'color scheme', 'mode'],
    anchor: 'appearance-theme',
  },
  {
    id: 'appearance-accent',
    section: 'appearance',
    titleKey: 'settings.appearance.accent',
    keywords: ['accent', 'color', 'brand', 'violet', 'blue', 'green', 'pink', 'orange', 'graphite'],
    anchor: 'appearance-accent',
  },
  {
    id: 'appearance-textsize',
    section: 'appearance',
    titleKey: 'settings.appearance.textSize',
    keywords: ['text', 'size', 'font', 'small', 'large', 'scale', 'typography'],
    anchor: 'appearance-textsize',
  },
  {
    id: 'appearance-transparency',
    section: 'appearance',
    titleKey: 'settings.appearance.reduceTransparency',
    keywords: ['transparency', 'glass', 'frosted', 'blur', 'solid'],
    anchor: 'appearance-transparency',
  },
  {
    id: 'appearance-contrast',
    section: 'appearance',
    titleKey: 'settings.appearance.increaseContrast',
    keywords: ['contrast', 'border', 'accessibility', 'a11y', 'visibility'],
    anchor: 'appearance-contrast',
  },

  // ── contact ──────────────────────────────────────────────────────────────────
  {
    id: 'contact-profile',
    section: 'contact',
    titleKey: 'settings.contactProfile.title',
    keywords: [
      'contact',
      'email',
      'phone',
      'name',
      'linkedin',
      'github',
      'website',
      'header',
      'resume',
      'cv',
      'photo',
      'profile',
      'address',
    ],
    anchor: 'contact-profile',
  },
  {
    id: 'contact-applicant',
    section: 'contact',
    titleKey: 'settings.applicant.title',
    keywords: [
      'salary',
      'start date',
      'notice',
      'remote',
      'hybrid',
      'applicant',
      'cover letter',
      'work',
      'compensation',
      'earliest',
    ],
    anchor: 'contact-applicant',
  },

  // ── ai ───────────────────────────────────────────────────────────────────────
  {
    id: 'ai-provider',
    section: 'ai',
    titleKey: 'settings.aiProvider.title',
    keywords: [
      'provider',
      'ollama',
      'openai',
      'claude',
      'anthropic',
      'gemini',
      'groq',
      'mistral',
      'api key',
      'local',
      'cloud',
      'model',
      'llm',
      'gpt',
    ],
    anchor: 'ai-provider',
  },
  {
    id: 'ai-tone',
    section: 'ai',
    titleKey: 'settings.outputTone.title',
    keywords: [
      'tone',
      'style',
      'professional',
      'casual',
      'formal',
      'creative',
      'writing',
      'output',
      'voice',
    ],
    anchor: 'ai-tone',
  },
  {
    id: 'ai-embeddings',
    section: 'ai',
    titleKey: 'settings.ai.embeddings.title',
    keywords: ['embeddings', 'search', 'matching', 'vector', 'semantic', 'index'],
    anchor: 'ai-embeddings',
  },
  {
    id: 'ai-company-research',
    section: 'ai',
    titleKey: 'settings.companyResearch.title',
    keywords: ['company', 'research', 'web search', 'brief', 'ollama key'],
    anchor: 'ai-company-research',
  },
  {
    id: 'ai-spend',
    section: 'ai',
    titleKey: 'settings.ai.spend.title',
    keywords: ['spend', 'cost', 'tokens', 'usage', 'budget', 'price', 'estimate'],
    anchor: 'ai-spend',
  },

  // ── job ──────────────────────────────────────────────────────────────────────
  {
    id: 'job-location',
    section: 'job',
    titleKey: 'settings.location.title',
    keywords: ['location', 'city', 'country', 'remote', 'place', 'geo', 'where'],
    anchor: 'job-location',
  },
  {
    id: 'job-techstack',
    section: 'job',
    titleKey: 'settings.techStack.title',
    keywords: [
      'tech',
      'stack',
      'skills',
      'programming',
      'language',
      'framework',
      'javascript',
      'typescript',
      'react',
      'python',
      'rust',
      'database',
    ],
    anchor: 'job-techstack',
  },
  {
    id: 'job-aggregator',
    section: 'job',
    titleKey: 'settings.aggregatorKeys.title',
    keywords: [
      'adzuna',
      'jsearch',
      'rapidapi',
      'aggregator',
      'job search',
      'api key',
      'search provider',
      'indeed',
      'jobs',
    ],
    anchor: 'job-aggregator',
  },

  // ── resume ───────────────────────────────────────────────────────────────────
  {
    id: 'resume-manage',
    section: 'resume',
    titleKey: 'settings.resume.title',
    keywords: [
      'resume',
      'cv',
      'upload',
      'pdf',
      'docx',
      'document',
      'default',
      'import',
      'ocr',
      'linkedin',
    ],
    anchor: 'resume-manage',
  },

  // ── accounts ─────────────────────────────────────────────────────────────────
  {
    id: 'accounts-boards',
    section: 'accounts',
    titleKey: 'settings.accounts.boardsTitle',
    keywords: [
      'accounts',
      'boards',
      'login',
      'session',
      'linkedin',
      'indeed',
      'glassdoor',
      'xing',
      'stepstone',
      'connect',
      'sign in',
    ],
    anchor: 'accounts-boards',
  },
  {
    id: 'accounts-extension',
    section: 'accounts',
    titleKey: 'settings.accounts.extension.title',
    keywords: [
      'extension',
      'browser',
      'chrome',
      'firefox',
      'pairing',
      'token',
      'bridge',
      'websocket',
      'plugin',
    ],
    anchor: 'accounts-extension',
  },
  {
    id: 'accounts-email-watch',
    section: 'accounts',
    titleKey: 'settings.accounts.emailWatch.title',
    keywords: [
      'email',
      'gmail',
      'imap',
      'app password',
      'confirmation',
      'auto-track',
      'watch',
      'inbox',
    ],
    anchor: 'accounts-email-watch',
  },

  // ── privacy ──────────────────────────────────────────────────────────────────
  {
    id: 'privacy-data',
    section: 'privacy',
    titleKey: 'settings.privacy.dataTitle',
    keywords: [
      'privacy',
      'data',
      'export',
      'import',
      'backup',
      'sign out',
      'clear',
      'history',
      'interactions',
      'gdpr',
    ],
    anchor: 'privacy-data',
  },
  {
    id: 'privacy-reset',
    section: 'privacy',
    titleKey: 'settings.privacy.resetApp',
    keywords: ['reset', 'factory', 'wipe', 'delete', 'fresh start', 'danger'],
    anchor: 'privacy-reset',
  },

  // ── performance ──────────────────────────────────────────────────────────────
  {
    id: 'performance-mode',
    section: 'performance',
    titleKey: 'settings.performanceMode.heading',
    keywords: [
      'performance',
      'memory',
      'ram',
      'speed',
      'low memory',
      'balanced',
      'animations',
      'blur',
      'aurora',
      'concurrency',
      'cache',
      'nebula',
    ],
    anchor: 'performance-mode',
  },

  // ── developer ────────────────────────────────────────────────────────────────
  {
    id: 'developer-tools',
    section: 'developer',
    titleKey: 'settings.developer.title',
    keywords: [
      'developer',
      'debug',
      'devtools',
      'console',
      'logs',
      'diagnostics',
      'export diagnostics',
      'verbose',
      'inspect',
    ],
    anchor: 'developer-tools',
  },

  // ── about ────────────────────────────────────────────────────────────────────
  {
    id: 'about-info',
    section: 'about',
    titleKey: 'settings.about.title',
    keywords: [
      'about',
      'version',
      'donate',
      'sponsor',
      'kofi',
      'paypal',
      'github',
      'support',
      'fund',
      'contribute',
    ],
    anchor: 'about-info',
  },
];
