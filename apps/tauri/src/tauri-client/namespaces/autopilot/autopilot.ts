import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { type AutopilotFocusEvent, type AutopilotStepEvent, EVENT_CHANNELS } from '@ajh/shared';
import type { AutopilotCreate, AutopilotUpdate } from '@ajh/shared/schemas';

import { asyncUnsub } from '../../utils.js';

export const autopilot = {
  list: () => invoke('autopilot_list'),
  get: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_get', { autopilotId }),
  create: (req: AutopilotCreate) => invoke('autopilot_create', { req }),
  update: ({ autopilotId, ...data }: { autopilotId: string } & AutopilotUpdate) =>
    invoke('autopilot_update', { autopilotId, req: data }),
  remove: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_remove', { autopilotId }),
  run: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_run', { autopilotId }),
  pause: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_pause', { autopilotId }),
  resume: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_resume', { autopilotId }),
  onStep: (handler: (event: AutopilotStepEvent) => void) =>
    asyncUnsub(() =>
      listen<AutopilotStepEvent>(EVENT_CHANNELS.autopilot.step, (e) => handler(e.payload))
    ),
  onFocus: (handler: (event: AutopilotFocusEvent) => void) =>
    asyncUnsub(() =>
      listen<AutopilotFocusEvent>(EVENT_CHANNELS.autopilot.focus, (e) => handler(e.payload))
    ),
};
