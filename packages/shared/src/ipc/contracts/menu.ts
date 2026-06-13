export interface MenuContract {
  /** Fired by the native menu (and other shell chrome) to deep-link into a
   *  route. `section` carries a settings sub-section when `route` is the
   *  settings page (e.g. `{ route: '/settings', section: 'ai' }`); `null`
   *  otherwise. */
  onNavigate(handler: (event: MenuNavigateEvent) => void): () => void;

  /** Fired by the native menu for app-level actions that aren't routes:
   *  trigger an update check or open the keyboard-shortcuts cheat-sheet. */
  onAction(handler: (event: MenuActionEvent) => void): () => void;

  /** Atomically take + clear the menu intent buffered by the shell while the
   *  window was hidden/minimized (close-to-tray). The shell's `emit` is
   *  fire-and-forget, so a `menu:navigate`/`menu:action` fired right after the
   *  window is un-hidden lands before the resumed webview re-attaches its
   *  listeners and is lost; the renderer pulls the buffered intent once its JS
   *  loop is live (on mount and on window focus/visibility-restore). Returns
   *  `null` when nothing is buffered. */
  takePending(): Promise<PendingMenuIntent | null>;
}

/** A menu intent buffered shell-side and pulled by the renderer. Discriminated
 *  by the same event name the shell would otherwise `emit`. */
export type PendingMenuIntent =
  | { event: 'menu:navigate'; payload: MenuNavigateEvent }
  | { event: 'menu:action'; payload: MenuActionEvent };

export interface MenuNavigateEvent {
  route: string;
  section: string | null;
  /** Optional in-page focus signal carried alongside the route. The native menu
   *  and tray omit it; the `ajh://settings/extension` deep link sets it to
   *  `'extension-token'` so the Accounts → Browser-extension section focuses the
   *  pairing token. Optional so omitting consumers (the native menu) still
   *  type-check. */
  focus?: 'extension-token';
}

export interface MenuActionEvent {
  action: 'check-updates' | 'shortcuts';
}
