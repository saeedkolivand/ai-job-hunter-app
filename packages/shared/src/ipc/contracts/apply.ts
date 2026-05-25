export interface ApplyContract {
  /** Enqueue an apply.job. Returns the job id; subscribe via jobs.onEvent
   *  to watch stages/progress stream back. */
  start(req: {
    board: string;
    url: string;

    coverLetter?: string;
    resumePath?: string;
    autoSubmit?: boolean;
  }): Promise<{ jobId: string }>;

  /** List supported appliers (boardId + display name). */
  catalog(): Promise<Array<{ id: string; displayName: string }>>;
}

export const APPLY_CHANNELS = {
  start: 'apply:start',
  catalog: 'apply:catalog',
} as const;
