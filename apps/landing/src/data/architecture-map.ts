// Single source of truth for the /architecture-map page (ported by execution
// from the former hand-authored public passthrough dashboard). The interactive
// map renders its static structure (clusters, nodes, edges) as prerendered JSX
// from these arrays; the pan/zoom/hover/filter engine is imperative.
//
// This file plus architecture-map/ (nodes per cluster, edges per flow, shared
// types) are the drift-guard target for scripts/check-landing-drift.mjs: every
// repo-relative path a node cites must exist on disk, and every contract-cluster
// node label (`<name>.ts`) must name a real packages/shared/src/ipc/contracts/
// file. Content is byte-faithful to the source (labels, sub, role/plain/path
// prose, findings) so the map can never silently lie about the architecture.

import { generationAutopilotEdges } from './architecture-map/edges-generation-autopilot';
import { generationFlowEdges } from './architecture-map/edges-generation-flow';
import { scrapeAnalyzeDocumentsEdges } from './architecture-map/edges-scrape-analyze-documents';
import { settingsPlatformEdges } from './architecture-map/edges-settings-platform';
import { clientNodes } from './architecture-map/nodes-client';
import { commandNodes } from './architecture-map/nodes-command';
import { contractNodes } from './architecture-map/nodes-contract';
import { dataNodes } from './architecture-map/nodes-data';
import { domainNodes } from './architecture-map/nodes-domain';
import { externalNodes } from './architecture-map/nodes-external';
import { hooksNodes } from './architecture-map/nodes-hooks';
import { infraNodes } from './architecture-map/nodes-infra';
import { promptsNodes } from './architecture-map/nodes-prompts';
import { providerNodes } from './architecture-map/nodes-provider';
import { scraperNodes } from './architecture-map/nodes-scraper';
import type { Bug, Cluster, Fix, MapEdge, MapNode } from './architecture-map/types';

export type {
  Bug,
  Cluster,
  ClusterColor,
  EdgeKind,
  Fix,
  MapEdge,
  MapNode,
} from './architecture-map/types';

export const clusters: readonly Cluster[] = [
  { id: 'client', label: 'Client / Renderer', x: 20, y: 20, w: 250, h: 1120, color: 'client' },
  { id: 'hooks', label: 'Service Hooks', x: 300, y: 20, w: 250, h: 1120, color: 'hooks' },
  {
    id: 'prompts',
    label: 'Prompts (LLM assembly)',
    x: 580,
    y: 20,
    w: 250,
    h: 1120,
    color: 'prompts',
  },
  { id: 'contract', label: 'Shared Contracts', x: 860, y: 20, w: 250, h: 1120, color: 'contract' },
  { id: 'command', label: 'Rust Commands', x: 1140, y: 20, w: 250, h: 1120, color: 'command' },
  { id: 'domain', label: 'Rust Domains', x: 1420, y: 20, w: 250, h: 1120, color: 'domain' },
  {
    id: 'scraper',
    label: 'Scrapers (SCRAPERS · 25)',
    x: 1700,
    y: 20,
    w: 472,
    h: 1120,
    color: 'scraper',
  },
  { id: 'provapp', label: 'AI Providers', x: 2220, y: 20, w: 250, h: 1120, color: 'provider' },
  { id: 'data', label: 'Data', x: 2500, y: 20, w: 250, h: 1120, color: 'data' },
  { id: 'external', label: 'External', x: 2780, y: 20, w: 256, h: 1120, color: 'external' },
  {
    id: 'infra',
    label: 'Platform Infra (single-owner · composed everywhere)',
    x: 1140,
    y: 1170,
    w: 1330,
    h: 130,
    color: 'infra',
  },
];

export const nodes: readonly MapNode[] = [
  ...clientNodes,
  ...hooksNodes,
  ...promptsNodes,
  ...contractNodes,
  ...commandNodes,
  ...domainNodes,
  ...scraperNodes,
  ...providerNodes,
  ...dataNodes,
  ...externalNodes,
  ...infraNodes,
];

export const edges: readonly MapEdge[] = [
  ...generationFlowEdges,
  ...generationAutopilotEdges,
  ...scrapeAnalyzeDocumentsEdges,
  ...settingsPlatformEdges,
];

// Node id → planned fixes / roadmap items (green badge + sidebar section).
export const FIXES: Readonly<Record<string, readonly Fix[]>> = {
  'ct-scrape': [
    {
      n: 1,
      t: 'Wire scrape.url into AI Generate — IPC exists, UI input not yet wired (ARCHITECTURE_STATUS.md:154)',
    },
  ],
  'cmd-scrape': [
    {
      n: 1,
      t: 'Wire scrape.url into AI Generate — IPC exists, UI input not yet wired (ARCHITECTURE_STATUS.md:154)',
    },
  ],
  'c-aigen': [
    { n: 1, t: 'Add a job-ad URL input that calls scrape.url (ARCHITECTURE_STATUS.md:154)' },
  ],
  'sc-linkedin': [
    {
      n: 2,
      t: 'LinkedIn official API integration — currently browser-only (ARCHITECTURE_STATUS.md:155)',
    },
  ],
  'db-datastore': [
    {
      n: 3,
      t: 'Cloud sync deferred — backup bundle + DataStore trait are the substrate (ARCHITECTURE_STATUS.md:159)',
    },
  ],
};

// Node id → known bugs (red badge + sidebar section).
export const KNOWN_BUGS: Readonly<Record<string, readonly Bug[]>> = {
  'cmd-match': [
    {
      sev: 'high',
      ref: 'ARCH_STATUS:47',
      t: 'Resume-job matcher is a Rust stub — was ✅ in old TS, not yet reimplemented',
    },
  ],
  'ct-match': [
    { sev: 'high', ref: 'ARCH_STATUS:47', t: 'Matcher contract has no real backend yet' },
  ],
  'h-use-match': [{ sev: 'high', ref: 'ARCH_STATUS:47', t: 'Match hook calls a stubbed backend' }],
  'd-autopilot': [
    {
      sev: 'med',
      ref: 'ARCH_STATUS:123',
      t: 'Autopilot find→rank→notify cadence varies by schedule (🚧)',
    },
  ],
};

// Notable findings shown in the default sidebar. Verbatim prose — the embedded
// <b> emphasis is rendered as React elements, never via innerHTML.
export const FINDINGS: readonly string[] = [
  'Code↔doc mismatch: the resume-job <b>matcher</b> is a Rust stub, though it was ✅ in the old TypeScript and the status doc still lists features as done (ARCHITECTURE_STATUS.md:47-50). Flagged red on cmd-match.',
  'Heaviest hot path: <b>AI generation</b> fans through packages/prompts (resume/cover builders + context-manager truncation + ProviderProfile + validators) before a single token streams — the red spine.',
  'Strong seam: a single-owner <b>platform band</b> (platform::config, net::http, error::AppError, observability::Span) plus two registries — SCRAPERS (25), ProviderId (8). Adding a board/provider is one module + one line; CI arch tests enforce ownership.',
  'No silent fallback: ai_provider::resolve() routes strictly by ProviderId — an unknown/mismatched provider is a hard error, never a quiet switch to Ollama.',
  'God nodes (edges intentionally not all drawn): cn() ~191, useAppClient() ~167, Button ~97. They touch nearly everything in their layer.',
  'tauri-client is a folder module (apps/desktop/src/tauri-client/index.ts); CLAUDE.md still cites the old single-file path.',
];

// Cluster/critical color → hex, keyed by ClusterColor plus 'critical'.
export const COLORS: Readonly<Record<string, string>> = {
  client: '#4ea1ff',
  hooks: '#7bd389',
  prompts: '#f5b942',
  contract: '#c792ea',
  command: '#ff7a45',
  domain: '#5ec8c8',
  scraper: '#7bd389',
  provider: '#c792ea',
  data: '#ffb86b',
  external: '#ff6b9d',
  infra: '#9aa7b3',
  critical: '#ff3860',
};

// Filter chips: [filter id, label].
export const CHIPS: readonly (readonly [string, string])[] = [
  ['overview', 'Overview'],
  ['aigenerate', 'AI Generate'],
  ['autopilot', 'Autopilot'],
  ['scrape', 'Scrape → Match'],
  ['analyze', 'Analyze'],
  ['documents', 'Documents'],
  ['settings', 'Settings'],
  ['all', 'Show all wires'],
  ['bugs', 'Roadmap & bugs'],
];

// Map legend rows — each is one or more colour-swatch + label pairs rendered
// in order (the "fix count / bug count" row has two).
export interface LegendSwatch {
  className: string;
  label: string;
}

export interface LegendRow {
  swatches: readonly LegendSwatch[];
}

export const LEGEND_ROWS: readonly LegendRow[] = [
  { swatches: [{ className: 'sw sw-critical', label: 'critical path' }] },
  { swatches: [{ className: 'sw sw-api', label: 'external / API call' }] },
  { swatches: [{ className: 'sw sw-db', label: 'DB read/write' }] },
  { swatches: [{ className: 'sw sw-mount', label: 'mount / register' }] },
  {
    swatches: [
      { className: 'dot dot-fix', label: 'fix count' },
      { className: 'dot dot-bug', label: 'bug count' },
    ],
  },
];

// #kbd-help dialog rows: an ordered sequence of <kbd> keys and plain text.
export type KbdHelpPart = { kbd: string } | { text: string };

export const KBD_HELP_ROWS: readonly (readonly KbdHelpPart[])[] = [
  [
    { kbd: 'drag' },
    { text: ' · ' },
    { kbd: '↑' },
    { kbd: '↓' },
    { kbd: '←' },
    { kbd: '→' },
    { text: ' pan' },
  ],
  [{ kbd: 'wheel' }, { text: ' · ' }, { kbd: '+' }, { kbd: '−' }, { text: ' zoom' }],
  [{ kbd: 'Tab' }, { text: ' focus next node' }],
  [{ kbd: 'Enter' }, { text: ' pin · ' }, { kbd: 'Esc' }, { text: ' clear' }],
  [
    { kbd: '0' },
    { text: ' / ' },
    { kbd: 'F' },
    { text: ' fit · ' },
    { kbd: '?' },
    { text: ' toggle help' },
  ],
];
