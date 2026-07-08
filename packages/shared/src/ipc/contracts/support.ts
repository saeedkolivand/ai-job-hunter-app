export interface SupportContract {
  /** Build and save a redacted diagnostics zip to the caller-supplied path */
  exportDiagnostics(
    dest: string
  ): Promise<{ success: true; path: string } | { success: false; error: string }>;
}

export const SUPPORT_CHANNELS = {
  exportDiagnostics: 'support:exportDiagnostics',
} as const;
