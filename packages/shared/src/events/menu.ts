// Wire now uses colon names (`menu:navigate`/`menu:action`). Dot-style renamed in this phase.
export const MENU_EVENTS = {
  navigate: 'menu:navigate',
  action: 'menu:action',
} as const;
