import type { AutopilotCreate, AutopilotUpdate } from '../../schemas/index.js';
import type { Autopilot } from '../../types/index.js';

export interface AutopilotContract {
  list(): Promise<Autopilot[]>;

  get(req: { autopilotId: string }): Promise<Autopilot | null>;

  create(req: AutopilotCreate): Promise<Autopilot>;

  update(req: { autopilotId: string } & AutopilotUpdate): Promise<Autopilot>;

  remove(req: { autopilotId: string }): Promise<void>;

  run(req: { autopilotId: string }): Promise<{ jobId: string }>;

  pause(req: { autopilotId: string }): Promise<void>;

  resume(req: { autopilotId: string }): Promise<void>;
}

export const AUTOPILOT_CHANNELS = {
  list: 'autopilot:list',
  get: 'autopilot:get',
  create: 'autopilot:create',
  update: 'autopilot:update',
  remove: 'autopilot:remove',
  run: 'autopilot:run',
  pause: 'autopilot:pause',
  resume: 'autopilot:resume',
} as const;
