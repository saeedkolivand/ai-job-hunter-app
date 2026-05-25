export interface ShortcutsContract {
  onCommandPalette(handler: () => void): () => void;
}

export const SHORTCUTS_CHANNELS = {
  onCommandPalette: 'shortcut:command-palette',
} as const;
