import type { AutopilotCreate, AutopilotUpdate } from '../../schemas/index.js';
import type { Autopilot, AutopilotRunStatus } from '../../types/index.js';

export interface AutopilotContract {
  list(): Promise<Autopilot[]>;

  get(req: { autopilotId: string }): Promise<Autopilot | null>;

  create(req: AutopilotCreate): Promise<Autopilot>;

  update(req: { autopilotId: string } & AutopilotUpdate): Promise<Autopilot>;

  remove(req: { autopilotId: string }): Promise<void>;

  /**
   * Run an autopilot now. The backend command *resolves* (does not reject) with
   * an `{ error }` payload on a scrape failure or unknown id, so callers MUST
   * inspect `error` — a resolved value is not proof of success. `jobId` is
   * present on every non-error outcome (success / cancel).
   *
   * `status` mirrors the outcome persisted on the record (`completed` /
   * `completedWithErrors` / `failed`) on a run that reached the record site, so
   * a caller can tell a run that found real jobs from one where every board
   * failed WITHOUT re-fetching the record. Absent on the early `{ error }` and
   * `{ cancelled }` outcomes.
   */
  run(req: {
    autopilotId: string;
  }): Promise<{ jobId?: string; error?: string; status?: AutopilotRunStatus }>;

  pause(req: { autopilotId: string }): Promise<void>;

  resume(req: { autopilotId: string }): Promise<void>;

  onStep(handler: (event: AutopilotStepEvent) => void): () => void;

  /** Fired by the shell (tray "New jobs" click or a validated deep link) to
   *  focus an autopilot's found-jobs panel. An empty `autopilotId` is a pure
   *  "refresh the list" signal (e.g. after a tray Pause-All) with no navigation. */
  onFocus(handler: (event: AutopilotFocusEvent) => void): () => void;

  /** Atomically take + clear the autopilot-focus intent buffered by the shell.
   *  A cold-start `ajh://autopilot/<id>` deep link fires the `autopilot:focus`
   *  emit during Rust setup, before the renderer's `useAutopilotFocusNavigation`
   *  listener attaches, so the event is lost; the shell buffers the id and the
   *  renderer pulls it once its JS loop is live (on mount + on the emitted
   *  event). The IPC response is reliable where the event was not. Resolves to
   *  the buffered `autopilotId`, or `null` when nothing is buffered (the common
   *  case — only set by a cold-start deep link). Mirrors `menu.takePending`. */
  takePendingFocus(): Promise<string | null>;
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
} as const;
