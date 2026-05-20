/**
 * In-memory store for live scraping results.
 * Jobs are stored temporarily during scraping and cleared when scraping completes.
 * This avoids unnecessary database writes while allowing instant UI updates.
 */
import type { JobPosting } from '@ajh/shared';

export class InMemoryJobStore {
  private jobs = new Map<string, JobPosting>();
  private sessionJobs = new Map<string, Set<string>>(); // sessionId -> job IDs

  /**
   * Add a job to the store for a specific scraping session.
   */
  add(sessionId: string, job: JobPosting): void {
    this.jobs.set(job.id, job);
    if (!this.sessionJobs.has(sessionId)) {
      this.sessionJobs.set(sessionId, new Set());
    }
    this.sessionJobs.get(sessionId)?.add(job.id);
  }

  /**
   * Get all jobs for a specific session.
   */
  getForSession(sessionId: string): JobPosting[] {
    const jobIds = this.sessionJobs.get(sessionId);
    if (!jobIds) return [];
    return Array.from(jobIds)
      .map((id) => this.jobs.get(id))
      .filter((job): job is JobPosting => job !== undefined);
  }

  /**
   * Get all jobs currently in the store.
   */
  getAll(): JobPosting[] {
    return Array.from(this.jobs.values());
  }

  /**
   * Get a specific job by ID.
   */
  get(id: string): JobPosting | undefined {
    return this.jobs.get(id);
  }

  /**
   * Clear all jobs for a specific session.
   */
  clearSession(sessionId: string): void {
    const jobIds = this.sessionJobs.get(sessionId);
    if (jobIds) {
      jobIds.forEach((id) => this.jobs.delete(id));
      this.sessionJobs.delete(sessionId);
    }
  }

  /**
   * Clear all jobs.
   */
  clearAll(): void {
    this.jobs.clear();
    this.sessionJobs.clear();
  }

  /**
   * Get the count of jobs in the store.
   */
  size(): number {
    return this.jobs.size;
  }
}
