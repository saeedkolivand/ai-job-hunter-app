/**

 * Typed IPC contract — the single source of truth for renderer <-> main calls.

 *

 * Capability-based: each namespace is a distinct capability the preload exposes.

 * Channels are namespaced; payloads are validated by Zod in the main handlers.

 *

 * This file re-exports all namespace contracts and combines them into the original

 * IpcContract, IPC_CHANNELS, and IpcChannel types for backward compatibility.

 */

import { AI_CHANNELS, type AiContract } from './ai.js';
import { AI_GENERATIONS_CHANNELS, type AiGenerationsContract } from './aiGenerations.js';
import { AUTOPILOT_CHANNELS, type AutopilotContract } from './autopilot.js';
import { BOARDS_CHANNELS, type BoardsContract } from './boards.js';
import { CLI_AGENTS_CHANNELS, type CliAgentsContract } from './cliAgents.js';
import { CONTACT_PROFILE_CHANNELS, type ContactProfileContract } from './contactProfile.js';
import { CREDENTIALS_CHANNELS, type CredentialsContract } from './credentials.js';
import { DATA_CHANNELS, type DataContract } from './data.js';
import { DIALOG_CHANNELS, type DialogContract } from './dialog.js';
import { DOCUMENTS_CHANNELS, type DocumentsContract } from './documents.js';
import { GEOCODE_CHANNELS, type GeocodeContract } from './geocode.js';
import { JOB_PREFERENCES_CHANNELS, type JobPreferencesContract } from './jobPreferences.js';
import { JOBS_CHANNELS, type JobsContract } from './jobs.js';
import { LINKEDIN_CHANNELS, type LinkedinContract } from './linkedin.js';
import { MATCH_CHANNELS, type MatchContract } from './match.js';
import type { MenuContract } from './menu.js';
import { PRIVACY_CHANNELS, type PrivacyContract } from './privacy.js';
import { REFERRALS_CHANNELS, type ReferralsContract } from './referrals.js';
import { RESUME_CHANNELS, type ResumeContract } from './resume.js';
import { SCRAPE_CHANNELS, type ScrapeContract } from './scrape.js';
import { SEARCH_CHANNELS, type SearchContract } from './search.js';
import { SHORTCUTS_CHANNELS, type ShortcutsContract } from './shortcuts.js';
import { SUPPORT_CHANNELS, type SupportContract } from './support.js';
import { SYSTEM_CHANNELS, type SystemContract } from './system.js';
import { UPDATER_CHANNELS, type UpdaterContract } from './updater.js';

// Combine all namespace contracts into the original IpcContract interface

export interface IpcContract {
  aiGenerations: AiGenerationsContract;
  system: SystemContract;
  jobs: JobsContract;
  ai: AiContract;
  documents: DocumentsContract;
  jobPreferences: JobPreferencesContract;
  contactProfile: ContactProfileContract;
  search: SearchContract;
  scrape: ScrapeContract;
  match: MatchContract;
  geocode: GeocodeContract;
  credentials: CredentialsContract;
  linkedin: LinkedinContract;
  boards: BoardsContract;
  cliAgents: CliAgentsContract;
  privacy: PrivacyContract;
  referrals: ReferralsContract;
  resume: ResumeContract;
  support: SupportContract;
  autopilot: AutopilotContract;
  menu: MenuContract;
  updater: UpdaterContract;
  shortcuts: ShortcutsContract;
  dialog: DialogContract;
  data: DataContract;
}

// Combine all channel constants into the original IPC_CHANNELS object

export const IPC_CHANNELS = {
  aiGenerations: AI_GENERATIONS_CHANNELS,
  system: SYSTEM_CHANNELS,
  jobs: JOBS_CHANNELS,
  ai: AI_CHANNELS,
  documents: DOCUMENTS_CHANNELS,
  jobPreferences: JOB_PREFERENCES_CHANNELS,
  contactProfile: CONTACT_PROFILE_CHANNELS,
  search: SEARCH_CHANNELS,
  scrape: SCRAPE_CHANNELS,
  match: MATCH_CHANNELS,
  geocode: GEOCODE_CHANNELS,
  credentials: CREDENTIALS_CHANNELS,
  linkedin: LINKEDIN_CHANNELS,
  boards: BOARDS_CHANNELS,
  cliAgents: CLI_AGENTS_CHANNELS,
  privacy: PRIVACY_CHANNELS,
  referrals: REFERRALS_CHANNELS,
  resume: RESUME_CHANNELS,
  support: SUPPORT_CHANNELS,
  autopilot: AUTOPILOT_CHANNELS,
  updater: UPDATER_CHANNELS,
  shortcuts: SHORTCUTS_CHANNELS,
  dialog: DIALOG_CHANNELS,
  data: DATA_CHANNELS,
} as const;

// Union type of all channel strings

export type IpcChannel =
  (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS][keyof (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS]];

/**
 * Renderer-side protocol version. Must match the version returned by
 * `system.getProtocolVersion()` from the Tauri shell. A mismatch at startup
 * indicates a partially-updated install and should be surfaced to the user.
 */
export const PROTOCOL_VERSION = '1.0.0';

// Re-export individual namespace contracts for direct imports if needed

export {
  AI_CHANNELS,
  type AiContract,
  type EmbeddingConfig,
  type EmbeddingSpaceInfo,
  type EmbeddingStatus,
} from './ai.js';
export {
  AI_GENERATIONS_CHANNELS,
  type AiGenerationRecord,
  type AiGenerationSaveRequest,
  type AiGenerationsContract,
  type AiGenerationUpdateRequest,
  type ApplicationAnswer,
} from './aiGenerations.js';
export {
  AUTOPILOT_CHANNELS,
  type AutopilotContract,
  type AutopilotFocusEvent,
  type AutopilotStepEvent,
} from './autopilot.js';
export {
  BOARDS_CHANNELS,
  type BoardsContract,
  type CookieImportOutcome,
  type CookieImportResult,
} from './boards.js';
export {
  CLI_AGENTS_CHANNELS,
  type CliAgentInstallResult,
  type CliAgentsContract,
  type CliAgentsStatus,
  type CliAgentStatus,
} from './cliAgents.js';
export {
  CONTACT_PROFILE_CHANNELS,
  type ContactLink,
  type ContactProfile,
  type ContactProfileContract,
  type LocalizedText,
} from './contactProfile.js';
export { CREDENTIALS_CHANNELS, type CredentialsContract } from './credentials.js';
export { DATA_CHANNELS, type DataContract } from './data.js';
export { DIALOG_CHANNELS, type DialogContract } from './dialog.js';
export {
  type ConfidenceLevel,
  type ContactFieldConflict,
  DOCUMENTS_CHANNELS,
  type DocumentsContract,
  type ResumeField,
  type SectionSummary,
  type SourceSpan,
  type StructuredResume,
  type TemplateRecommendation,
  type TemplateRecommendSignals,
} from './documents.js';
export { GEOCODE_CHANNELS, type GeocodeContract } from './geocode.js';
export { JOB_PREFERENCES_CHANNELS, type JobPreferencesContract } from './jobPreferences.js';
export { JOBS_CHANNELS, type JobsContract } from './jobs.js';
export { LINKEDIN_CHANNELS, type LinkedinContract } from './linkedin.js';
export { MATCH_CHANNELS, type MatchContract } from './match.js';
export { type MenuActionEvent, type MenuContract, type MenuNavigateEvent } from './menu.js';
export { PRIVACY_CHANNELS, type PrivacyContract } from './privacy.js';
export {
  type ReferralChannel,
  type ReferralContact,
  REFERRALS_CHANNELS,
  type ReferralsContract,
  type ReferralStatus,
  type ReferralUpsertRequest,
} from './referrals.js';
export { RESUME_CHANNELS, type ResumeContract } from './resume.js';
export { SCRAPE_CHANNELS, type ScrapeContract } from './scrape.js';
export { SEARCH_CHANNELS, type SearchContract } from './search.js';
export { SHORTCUTS_CHANNELS, type ShortcutsContract } from './shortcuts.js';
export { SUPPORT_CHANNELS, type SupportContract } from './support.js';
export { SYSTEM_CHANNELS, type SystemContract } from './system.js';
export {
  type ChangelogRelease,
  type ChangelogResult,
  UPDATER_CHANNELS,
  type UpdaterContract,
} from './updater.js';
