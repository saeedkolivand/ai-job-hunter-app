/**
 * Event-only namespace (like `MenuContract`): the shell pushes the
 * `shortcut:command-palette` event and the renderer subscribes. There are no
 * request/response channels, so this file exports no `SHORTCUTS_CHANNELS` const
 * and the namespace is intentionally absent from `IPC_CHANNELS`. The event name
 * lives in the centralized `EVENT_CHANNELS` registry under
 * `packages/shared/src/events/`.
 */
export interface ShortcutsContract {
  onCommandPalette(handler: () => void): () => void;
}
