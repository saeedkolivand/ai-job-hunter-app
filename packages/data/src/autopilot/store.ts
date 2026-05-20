/**
 * AutopilotStore — NeDB-backed CRUD for Autopilots.
 */
import { randomUUID } from 'node:crypto';
import type Datastore from '@seald-io/nedb';
import type { Autopilot, AutopilotStatus, AutopilotCreate, AutopilotUpdate } from '@ajh/shared';

export class AutopilotStore {
  constructor(private readonly col: Datastore) {
    col.ensureIndex({ fieldName: 'status' });
    col.ensureIndex({ fieldName: 'schedule' });
  }

  async list(): Promise<Autopilot[]> {
    return new Promise((resolve, reject) => {
      this.col
        .find({})
        .sort({ createdAt: -1 })
        .exec((err, docs) => {
          if (err) reject(err);
          else resolve(docs as Autopilot[]);
        });
    });
  }

  async get(autopilotId: string): Promise<Autopilot | null> {
    return new Promise((resolve, reject) => {
      this.col.findOne({ _id: autopilotId }, (err, doc) => {
        if (err) reject(err);
        else resolve(doc as Autopilot | null);
      });
    });
  }

  async create(input: AutopilotCreate): Promise<Autopilot> {
    const now = Date.now();
    const doc: Autopilot = {
      _id: randomUUID(),
      name: input.name,
      status: 'active',
      target: input.target,
      filter: input.filter,
      action: input.action,
      schedule: input.schedule,
      resumeText: input.resumeText,
      coverLetter: input.coverLetter,
      autoSubmit: input.autoSubmit ?? false,
      createdAt: now,
      updatedAt: now,
      totalFound: 0,
      totalApplied: 0,
    };
    return new Promise((resolve, reject) => {
      this.col.insert(doc, (err, inserted) => {
        if (err) reject(err);
        else resolve(inserted as Autopilot);
      });
    });
  }

  async update(autopilotId: string, patch: AutopilotUpdate): Promise<Autopilot> {
    const update = { $set: { ...patch, updatedAt: Date.now() } };
    return new Promise((resolve, reject) => {
      this.col.update({ _id: autopilotId }, update, {}, (err) => {
        if (err) {
          reject(err);
          return;
        }
        this.col.findOne({ _id: autopilotId }, (err2, doc) => {
          if (err2) reject(err2);
          else resolve(doc as Autopilot);
        });
      });
    });
  }

  async remove(autopilotId: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.col.remove({ _id: autopilotId }, {}, (err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }

  async setStatus(autopilotId: string, status: AutopilotStatus): Promise<void> {
    return new Promise((resolve, reject) => {
      this.col.update(
        { _id: autopilotId },
        { $set: { status, updatedAt: Date.now() } },
        {},
        (err) => {
          if (err) reject(err);
          else resolve();
        }
      );
    });
  }

  async recordRun(
    autopilotId: string,
    jobId: string,
    found: number,
    applied: number
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      this.col.update(
        { _id: autopilotId },
        {
          $set: { lastRunAt: Date.now(), lastRunJobId: jobId, updatedAt: Date.now() },
          $inc: { totalFound: found, totalApplied: applied },
        },
        {},
        (err) => {
          if (err) reject(err);
          else resolve();
        }
      );
    });
  }

  async listBySchedule(schedule: string): Promise<Autopilot[]> {
    return new Promise((resolve, reject) => {
      this.col.find({ status: 'active', schedule }, (err: Error | null, docs: unknown[]) => {
        if (err) reject(err);
        else resolve(docs as Autopilot[]);
      });
    });
  }
}
