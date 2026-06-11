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

import { ai } from './namespaces/ai/index.js';
import { aiGenerations } from './namespaces/aiGenerations/index.js';
import { applications } from './namespaces/applications/index.js';
import { autopilot } from './namespaces/autopilot/index.js';
import { boards } from './namespaces/boards/index.js';
import { cliAgents } from './namespaces/cliAgents/index.js';
import { contactProfile } from './namespaces/contactProfile/index.js';
import { credentials } from './namespaces/credentials/index.js';
import { data } from './namespaces/data/index.js';
import { dialog } from './namespaces/dialog/index.js';
import { documents } from './namespaces/documents/index.js';
import { geocode } from './namespaces/geocode/index.js';
import { jobPreferences } from './namespaces/jobPreferences/index.js';
import { jobs } from './namespaces/jobs/index.js';
import { linkedin } from './namespaces/linkedin/index.js';
import { match } from './namespaces/match/index.js';
import { menu } from './namespaces/menu/index.js';
import { privacy } from './namespaces/privacy/index.js';
import { referrals } from './namespaces/referrals/index.js';
import { resume } from './namespaces/resume/index.js';
import { scrape } from './namespaces/scrape/index.js';
import { search } from './namespaces/search/index.js';
import { shortcuts } from './namespaces/shortcuts/index.js';
import { support } from './namespaces/support/index.js';
import { system } from './namespaces/system/index.js';
import { updater } from './namespaces/updater/index.js';

export function createTauriInvokeClient(): AppClient {
  return {
    aiGenerations: aiGenerations as AppClient['aiGenerations'],
    applications: applications as AppClient['applications'],
    system: system as AppClient['system'],
    jobs: jobs as AppClient['jobs'],
    ai: ai as AppClient['ai'],
    documents: documents as AppClient['documents'],
    jobPreferences: jobPreferences as AppClient['jobPreferences'],
    contactProfile: contactProfile as AppClient['contactProfile'],
    search: search as AppClient['search'],
    scrape: scrape as AppClient['scrape'],
    match: match as AppClient['match'],
    geocode: geocode as AppClient['geocode'],
    credentials: credentials as AppClient['credentials'],
    linkedin: linkedin as AppClient['linkedin'],
    boards: boards as AppClient['boards'],
    cliAgents: cliAgents as AppClient['cliAgents'],
    privacy: privacy as AppClient['privacy'],
    referrals: referrals as AppClient['referrals'],
    updater: updater as AppClient['updater'],
    shortcuts: shortcuts as AppClient['shortcuts'],
    resume: resume as AppClient['resume'],
    support: support as AppClient['support'],
    autopilot: autopilot as AppClient['autopilot'],
    menu: menu as AppClient['menu'],
    dialog: dialog as AppClient['dialog'],
    data: data as AppClient['data'],
  } satisfies AppClient;
}
