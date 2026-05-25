export interface DialogContract {
  openFiles(opts?: {
    multiple?: boolean;
    filters?: Array<{ name: string; extensions: string[] }>;
  }): Promise<string[]>;
}

export const DIALOG_CHANNELS = {
  /** Open a native file picker — implemented by Electron dialog on desktop,
   *  Tauri dialog plugin in the Tauri shell. */
  openFiles: 'dialog:open-files',
} as const;
