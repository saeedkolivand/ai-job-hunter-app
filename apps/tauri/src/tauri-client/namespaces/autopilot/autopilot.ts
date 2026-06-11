import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { onAction } from '@tauri-apps/plugin-notification';

import type { AutopilotCreate, AutopilotUpdate } from '@ajh/shared/schemas';

import { asyncUnsub } from '../../utils.js';

export interface AutopilotStepEvent {
  jobId: string;
  autopilotId: string;
  step: string;
  detail: string;
}

export interface AutopilotFocusEvent {
  autopilotId: string;
}

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
    asyncUnsub(() => listen<AutopilotStepEvent>('autopilot.step', (e) => handler(e.payload))),
  onFocus: (handler: (event: AutopilotFocusEvent) => void) =>
    asyncUnsub(() => listen<AutopilotFocusEvent>('autopilot.focus', (e) => handler(e.payload))),
  // Clicking the OS notification (autopilot is the only notification source) →
  // open the autopilot page. Body-click fires `onAction` on macOS + packaged
  // Windows; best-effort on Linux. The payload is unused — any click navigates.
  onNotificationClick: (handler: () => void) =>
    asyncUnsub(() =>
      onAction(() => handler()).then((listener) => () => void listener.unregister())
    ),
  // Surfaces the hidden window + focuses the last autopilot; the shell re-emits
  // the existing `autopilot.focus` event for that id (which onFocus turns into
  // navigation). Called when the in-app "new jobs" notification is clicked.
  notificationClicked: () => invoke('autopilot_notification_clicked'),
};
