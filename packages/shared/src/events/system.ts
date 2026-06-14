// OS-driven system signals the shell pushes to the renderer. `accentChanged`
// fires when the Windows personalization accent color changes (WinRT
// `UISettings::ColorValuesChanged`); the renderer re-pulls `system_accent_color`
// and re-applies the theme when the accent source is 'system'. New system events
// must keep the `system:` wire prefix.
export const SYSTEM_EVENTS = {
  accentChanged: 'system:accentChanged',
} as const;
