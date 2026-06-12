// COLON target names. Wire is still dot today (`menu.navigate`/`menu.action`);
// a later phase renames the Rust/TS wire from dot -> colon.
export const MENU_EVENTS = {
  navigate: 'menu:navigate',
  action: 'menu:action',
} as const;
