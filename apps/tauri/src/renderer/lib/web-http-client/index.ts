/**
 * createWebHttpClient — Web HTTP Adapter for AppClient.
 *
 * This is the third and final adapter in the three-path architecture:
 *
 *   React features
 *     → AppClient (typed commands + job events)
 *       → createDesktopIpcClient()   desktop Electron (today)
 *       → createTauriInvokeClient()  Tauri shell     (spike done)
 *       → createWebHttpClient()      Web / REST      (this file)
 *
 * The web adapter talks to a running runtime server (the same Node.js
 * processes that run as Tauri sidecars can also run as standalone HTTP
 * services). This lets the React app deploy on the web without any native
 * shell — the runtime server handles scraping, AI, and data behind an
 * authenticated REST + WebSocket API.
 *
 * ── Protocol ────────────────────────────────────────────────────────────────
 *  Commands:   POST /api/<namespace>/<method>
 *              Content-Type: application/json
 *              Body: request payload
 *              Response: application/json result
 *
 *  Streaming:  GET /api/events/<channel>  (Server-Sent Events)
 *              Returns text/event-stream, each event is JSON ScraperEvent
 *
 * ── Authentication ───────────────────────────────────────────────────────────
 *  The runtime server is expected to run on localhost and accept a
 *  per-launch random token passed in the Authorization header. Bind to
 *  127.0.0.1 only — never expose to a network interface without a proxy
 *  that enforces auth.
 *
 * ── Status ───────────────────────────────────────────────────────────────────
 *  This is a documented implementation skeleton. All commands are wired
 *  and typed — wire up the actual runtime server URL to get a working
 *  web client. Streaming event subscriptions use EventSource.
 *
 *  Replace the RUNTIME_BASE_URL below or pass it as a constructor argument.
 */
import type { AppClient } from '@/lib/app-client';

import { ai } from './ai.js';
import { apply } from './apply.js';
import { autopilot } from './autopilot.js';
import { boards } from './boards.js';
import { conversations } from './conversations.js';
import { credentials } from './credentials.js';
import { dialog } from './dialog.js';
import { documents } from './documents.js';
import { geocode } from './geocode.js';
import { jobPreferences } from './jobPreferences.js';
import { jobs } from './jobs.js';
import { linkedin } from './linkedin.js';
import { match } from './match.js';
import { privacy } from './privacy.js';
import { resume } from './resume.js';
import { scrape } from './scrape.js';
import { search } from './search.js';
import { shortcuts } from './shortcuts.js';
import { support } from './support.js';
import { system } from './system.js';
import { updater } from './updater.js';
import type { WebHttpClientOptions } from './utils.js';

export function createWebHttpClient(opts: WebHttpClientOptions): AppClient {
  return {
    system: system(opts) as AppClient['system'],
    jobs: jobs(opts) as AppClient['jobs'],
    ai: ai(opts) as AppClient['ai'],
    documents: documents(opts) as AppClient['documents'],
    jobPreferences: jobPreferences(opts) as AppClient['jobPreferences'],
    search: search(opts) as AppClient['search'],
    scrape: scrape(opts) as AppClient['scrape'],
    match: match(opts) as AppClient['match'],
    geocode: geocode(opts) as AppClient['geocode'],
    credentials: credentials(opts) as AppClient['credentials'],
    linkedin: linkedin(opts) as AppClient['linkedin'],
    boards: boards(opts) as AppClient['boards'],
    privacy: privacy(opts) as AppClient['privacy'],
    apply: apply(opts) as AppClient['apply'],
    updater: updater(opts) as AppClient['updater'],
    shortcuts: shortcuts(opts) as AppClient['shortcuts'],
    resume: resume(opts) as AppClient['resume'],
    support: support(opts) as AppClient['support'],
    conversations: conversations(opts) as AppClient['conversations'],
    autopilot: autopilot(opts) as AppClient['autopilot'],
    dialog: dialog(opts) as AppClient['dialog'],
  } satisfies AppClient;
}

export type { WebHttpClientOptions } from './utils.js';
