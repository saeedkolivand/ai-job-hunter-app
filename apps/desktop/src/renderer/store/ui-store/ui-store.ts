import { create } from 'zustand';

/**
 * Transient, app-global UI flags that aren't tied to a single route (which would
 * live in `session-store`) or to remote data (React Query). Lets shell chrome —
 * the native menu, keyboard shortcuts — drive overlays mounted once in the root
 * layout.
 */
interface UiState {
  /** The keyboard-shortcuts cheat-sheet. Toggled by the `?` key and opened by
   *  the native "Keyboard Shortcuts" menu item. */
  shortcutsOpen: boolean;
  setShortcutsOpen: (open: boolean) => void;
  /** The notification-center inbox dropdown. Toggled by the titlebar bell and
   *  opened by the `notifications:open` event (OS-banner / tray click). */
  notificationsOpen: boolean;
  setNotificationsOpen: (open: boolean) => void;
  /** One-shot focus request for the Accounts → Browser-extension pairing token.
   *  Set by the `ajh://settings/extension` deep link (via the `menu:navigate`
   *  handler) and consumed + cleared by `ExtensionBridgeSection`, which scrolls
   *  the token field into view and gives it a one-shot highlight. */
  extensionTokenFocus: boolean;
  setExtensionTokenFocus: (focus: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  shortcutsOpen: false,
  setShortcutsOpen: (open) => set({ shortcutsOpen: open }),
  notificationsOpen: false,
  setNotificationsOpen: (open) => set({ notificationsOpen: open }),
  extensionTokenFocus: false,
  setExtensionTokenFocus: (focus) => set({ extensionTokenFocus: focus }),
}));
