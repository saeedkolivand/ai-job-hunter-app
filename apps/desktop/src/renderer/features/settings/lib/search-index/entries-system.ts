import type { SearchEntry } from './search-entry';

/** Performance, developer and about settings. */
export const SYSTEM_ENTRIES: SearchEntry[] = [
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
  {
    id: 'developer-agent-cli',
    section: 'developer',
    titleKey: 'settings.developer.agentCli.title',
    keywords: [
      'mcp',
      'cli',
      'agent',
      'claude code',
      'codex',
      'model context protocol',
      'ajh-tauri',
      'terminal',
      'command line',
      'cursor',
      'claude desktop',
      'windsurf',
      'mcpservers',
      'json',
    ],
    anchor: 'developer-agent-cli',
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
