/**
 * Centralized event-channel registry — the single source of truth for the
 * Tauri push events the shell emits and the renderer subscribes to. Mirrors the
 * `IPC_CHANNELS` pattern for request/response channels, but for one-way events.
 *
 * Event names use the COLON convention (`ns:event`) as the target wire format.
 * All channels now emit colon names on the wire (e.g. `menu:navigate`). This
 * registry is the source of truth; the lock test guards it.
 *
 * BARREL NOTE: the package barrel re-exports this file with `export *`, alongside
 * `types/index.js` and `ipc/contracts/index.js`. So this file must NOT re-export
 * any payload type already exported by those — only `import type` them. Only the
 * NEW types defined here (and the `*_EVENTS` consts) are exported.
 */
import type { AgentStepEvent } from '../ipc/contracts/agent.js';
import type { ApplicationChangedEvent } from '../ipc/contracts/applications.js';
import type { AutopilotFocusEvent, AutopilotStepEvent } from '../ipc/contracts/autopilot.js';
import type { MenuActionEvent, MenuNavigateEvent } from '../ipc/contracts/menu.js';
import type { AiStreamChunk, JobEvent, NotificationToast } from '../types/index.js';
import { AGENT_EVENTS } from './agent.js';
import { AI_EVENTS } from './ai.js';
import { APPLICATIONS_EVENTS } from './applications.js';
import { AUTOPILOT_EVENTS } from './autopilot.js';
import { BOARDS_EVENTS, type BoardsLoginStatusEvent } from './boards.js';
import { JOBS_EVENTS } from './jobs.js';
import { MENU_EVENTS } from './menu.js';
import { NOTIFICATIONS_EVENTS } from './notifications.js';
import { SCRAPE_EVENTS, type ScrapeItemEvent, type ScrapeProgressEvent } from './scrape.js';
import { SHORTCUTS_EVENTS } from './shortcuts.js';
import { SYSTEM_EVENTS } from './system.js';
import { UPDATER_EVENTS } from './updater.js';

// Combine all namespace event constants into one registry.
export const EVENT_CHANNELS = {
  agent: AGENT_EVENTS,
  ai: AI_EVENTS,
  jobs: JOBS_EVENTS,
  applications: APPLICATIONS_EVENTS,
  notifications: NOTIFICATIONS_EVENTS,
  updater: UPDATER_EVENTS,
  menu: MENU_EVENTS,
  autopilot: AUTOPILOT_EVENTS,
  scrape: SCRAPE_EVENTS,
  boards: BOARDS_EVENTS,
  shortcuts: SHORTCUTS_EVENTS,
  system: SYSTEM_EVENTS,
} as const;

// Union type of all event-channel wire strings.
export type EventChannel =
  (typeof EVENT_CHANNELS)[keyof typeof EVENT_CHANNELS][keyof (typeof EVENT_CHANNELS)[keyof typeof EVENT_CHANNELS]];

/**
 * Map of event wire name -> payload type. Keyed by the WIRE name (the value in
 * the registry), kept in 1:1 sync with `EVENT_CHANNELS` by the lock test.
 */
export interface AppEvents {
  'agent:step': AgentStepEvent;
  'ai:stream': AiStreamChunk;
  'jobs:event': JobEvent;
  'applications:changed': ApplicationChangedEvent;
  'notifications:changed': void;
  'notifications:open': void;
  'notifications:toast': NotificationToast;
  'updater:status': unknown;
  'menu:navigate': MenuNavigateEvent;
  'menu:action': MenuActionEvent;
  'autopilot:focus': AutopilotFocusEvent;
  'autopilot:step': AutopilotStepEvent;
  'scrape:progress': ScrapeProgressEvent;
  'scrape:item': ScrapeItemEvent;
  'boards:login-status': BoardsLoginStatusEvent;
  'shortcut:command-palette': void;
  'system:accentChanged': void;
}

export {
  AGENT_EVENTS,
  AI_EVENTS,
  APPLICATIONS_EVENTS,
  AUTOPILOT_EVENTS,
  BOARDS_EVENTS,
  type BoardsLoginStatusEvent,
  JOBS_EVENTS,
  MENU_EVENTS,
  NOTIFICATIONS_EVENTS,
  SCRAPE_EVENTS,
  type ScrapeItemEvent,
  type ScrapeProgressEvent,
  SHORTCUTS_EVENTS,
  SYSTEM_EVENTS,
  UPDATER_EVENTS,
};
