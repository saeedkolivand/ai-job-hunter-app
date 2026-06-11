export interface MenuContract {
  /** Fired by the native menu (and other shell chrome) to deep-link into a
   *  route. `section` carries a settings sub-section when `route` is the
   *  settings page (e.g. `{ route: '/settings', section: 'ai' }`); `null`
   *  otherwise. */
  onNavigate(handler: (event: MenuNavigateEvent) => void): () => void;

  /** Fired by the native menu for app-level actions that aren't routes:
   *  trigger an update check or open the keyboard-shortcuts cheat-sheet. */
  onAction(handler: (event: MenuActionEvent) => void): () => void;
}

export interface MenuNavigateEvent {
  route: string;
  section: string | null;
}

export interface MenuActionEvent {
  action: 'check-updates' | 'shortcuts';
}
