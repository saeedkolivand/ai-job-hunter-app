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

  onStep(handler: (event: AutopilotStepEvent) => void): () => void;

  /** Fired by the shell (tray "New jobs" click or a validated deep link) to
   *  focus an autopilot's found-jobs panel. An empty `autopilotId` is a pure
   *  "refresh the list" signal (e.g. after a tray Pause-All) with no navigation. */
  onFocus(handler: (event: AutopilotFocusEvent) => void): () => void;
}

export interface AutopilotStepEvent {
  jobId: string;
  autopilotId: string;
  step: string;
  detail: string;
}

export interface AutopilotFocusEvent {
  autopilotId: string;
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
  focus: 'autopilot:focus',
} as const;
