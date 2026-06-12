// Intentional asymmetry: the namespace key is `shortcuts` (plural) but wire
// names use the singular `shortcut:` prefix to match Rust global-shortcut
// registration (e.g. `shortcut:command-palette`). New shortcut events must
// keep the `shortcut:` wire prefix.
export const SHORTCUTS_EVENTS = {
  commandPalette: 'shortcut:command-palette',
} as const;
