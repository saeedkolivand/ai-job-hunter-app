import type { Applier } from './base.js';
import { LinkedInApplier } from './boards/linkedin.js';
import { IndeedApplier } from './boards/indeed.js';
import { WorkdayApplier } from './boards/workday.js';
import { GreenhouseApplier } from './boards/greenhouse.js';

export class ApplierRegistry {
  private readonly appliers = new Map<string, Applier>();
  constructor() {
    [
      new LinkedInApplier(),
      new IndeedApplier(),
      new GreenhouseApplier(),
      new WorkdayApplier(),
    ].forEach((a) => this.register(a));
  }
  register(a: Applier): void {
    this.appliers.set(a.boardId, a);
  }
  get(boardId: string): Applier | undefined {
    return this.appliers.get(boardId);
  }
  list(): Applier[] {
    return [...this.appliers.values()];
  }
  catalog(): Array<{ id: string; displayName: string }> {
    return this.list().map((a) => ({ id: a.boardId, displayName: a.displayName }));
  }
}
