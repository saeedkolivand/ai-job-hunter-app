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
}

export const useUiStore = create<UiState>((set) => ({
  shortcutsOpen: false,
  setShortcutsOpen: (open) => set({ shortcutsOpen: open }),
}));
