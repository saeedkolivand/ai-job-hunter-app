/**
 * Task scheduler — periodic + delayed tasks (e.g. re-index, garbage collect,
 * model unload after idle, scrape refresh windows).
 */
import { createLogger, type Logger } from '../logger.js';

type Task = () => void | Promise<void>;

interface Scheduled {
  id: string;
  handle: NodeJS.Timeout;
  cancel(): void;
}

export class TaskScheduler {
  private readonly logger: Logger = createLogger('scheduler');
  private readonly tasks = new Map<string, Scheduled>();

  every(id: string, intervalMs: number, task: Task): void {
    this.cancel(id);
    const handle = setInterval(() => {
      Promise.resolve(task()).catch((err) =>
        this.logger.error({ id, err }, 'periodic task failed')
      );
    }, intervalMs);
    this.tasks.set(id, { id, handle, cancel: () => clearInterval(handle) });
  }

  after(id: string, delayMs: number, task: Task): void {
    this.cancel(id);
    const handle = setTimeout(() => {
      this.tasks.delete(id);
      Promise.resolve(task()).catch((err) => this.logger.error({ id, err }, 'delayed task failed'));
    }, delayMs);
    this.tasks.set(id, { id, handle, cancel: () => clearTimeout(handle) });
  }

  cancel(id: string): void {
    this.tasks.get(id)?.cancel();
    this.tasks.delete(id);
  }

  cancelAll(): void {
    for (const t of this.tasks.values()) t.cancel();
    this.tasks.clear();
  }
}
