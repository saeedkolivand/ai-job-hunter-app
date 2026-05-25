/**
 * TauriInvokeClient — implements AppClient using Tauri v2 invoke + listen.
 *
 * Used by apps/tauri/src/main.tsx instead of the Electron window.api bridge.
 * The shape is structurally identical to the Electron client so all service
 * hooks work without modification.
 *
 * Event subscriptions: Tauri's listen() is async. We return a sync cleanup
 * function that cancels the listener once the promise resolves. This matches
 * the Electron preload pattern.
 *
 * Unimplemented commands return stubs (null / []) so the UI degrades
 * gracefully while parity is being built up incrementally.
 */
import type { AppClient } from '@/lib/app-client';

import { ai } from './namespaces/ai.js';
import { apply } from './namespaces/apply.js';
import { autopilot } from './namespaces/autopilot.js';
import { boards } from './namespaces/boards.js';
import { conversations } from './namespaces/conversations.js';
import { credentials } from './namespaces/credentials.js';
import { dialog } from './namespaces/dialog.js';
import { documents } from './namespaces/documents.js';
import { geocode } from './namespaces/geocode.js';
import { jobPreferences } from './namespaces/jobPreferences.js';
import { jobs } from './namespaces/jobs.js';
import { linkedin } from './namespaces/linkedin.js';
import { match } from './namespaces/match.js';
import { privacy } from './namespaces/privacy.js';
import { resume } from './namespaces/resume.js';
import { scrape } from './namespaces/scrape.js';
import { search } from './namespaces/search.js';
import { shortcuts } from './namespaces/shortcuts.js';
import { support } from './namespaces/support.js';
import { system } from './namespaces/system.js';
import { updater } from './namespaces/updater.js';

export function createTauriInvokeClient(): AppClient {
  return {
    system: system as AppClient['system'],
    jobs: jobs as AppClient['jobs'],
    ai: ai as AppClient['ai'],
    documents: documents as AppClient['documents'],
    jobPreferences: jobPreferences as AppClient['jobPreferences'],
    search: search as AppClient['search'],
    scrape: scrape as AppClient['scrape'],
    match: match as AppClient['match'],
    geocode: geocode as AppClient['geocode'],
    credentials: credentials as AppClient['credentials'],
    linkedin: linkedin as AppClient['linkedin'],
    boards: boards as AppClient['boards'],
    privacy: privacy as AppClient['privacy'],
    apply: apply as AppClient['apply'],
    updater: updater as AppClient['updater'],
    shortcuts: shortcuts as AppClient['shortcuts'],
    resume: resume as AppClient['resume'],
    support: support as AppClient['support'],
    conversations: conversations as AppClient['conversations'],
    autopilot: autopilot as AppClient['autopilot'],
    dialog: dialog as AppClient['dialog'],
  } satisfies AppClient;
}
