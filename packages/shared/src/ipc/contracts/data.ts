/** Full app-data backup & restore (all persistent stores). */
export interface DataContract {
  /** Export all user data to a user-chosen JSON file. */
  export(): Promise<{ success: boolean; filePath?: string; error?: string }>;

  /** Restore all user data from a user-chosen backup file (replace semantics). */
  import(): Promise<{
    success: boolean;
    /** True when one or more stores failed to restore (others may have succeeded). */
    partial?: boolean;
    imported?: Record<string, number | { error: string }>;
    error?: string;
  }>;
}

export const DATA_CHANNELS = {
  export: 'data:export',
  import: 'data:import',
} as const;
