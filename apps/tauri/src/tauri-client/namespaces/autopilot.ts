import { invoke } from '@tauri-apps/api/core';

export const autopilot = {
  list: () => invoke('autopilot_list'),
  get: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_get', { autopilotId }),
  create: (req: unknown) => invoke('autopilot_create', { req }),
  update: ({ autopilotId, ...data }: { autopilotId: string } & Record<string, unknown>) =>
    invoke('autopilot_update', { autopilotId, req: data }),
  remove: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_remove', { autopilotId }),
  run: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_run', { autopilotId }),
  pause: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_pause', { autopilotId }),
  resume: ({ autopilotId }: { autopilotId: string }) => invoke('autopilot_resume', { autopilotId }),
};
