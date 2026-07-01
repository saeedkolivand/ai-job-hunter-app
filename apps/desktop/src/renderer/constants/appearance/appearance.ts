import { Monitor, Moon, Sun } from 'lucide-react';

import type { ColorScheme } from '@ajh/ui';

export const SCHEMES: { id: ColorScheme; icon: typeof Sun; labelKey: string }[] = [
  { id: 'light', icon: Sun, labelKey: 'settings.appearance.light' },
  { id: 'dark', icon: Moon, labelKey: 'settings.appearance.dark' },
  { id: 'system', icon: Monitor, labelKey: 'settings.appearance.system' },
];

// macOS-style preset accents the user can pick manually. 'default' (handled
// separately) keeps the shipped, per-scheme-tuned violet; each preset here is a
// fixed hex applied to both schemes via the theme engine's accent applier.
export const ACCENTS: { id: string; color: string; color2: string; labelKey: string }[] = [
  {
    id: 'violet',
    color: '#a855f7',
    color2: '#6366f1',
    labelKey: 'settings.appearance.accentViolet',
  },
  { id: 'blue', color: '#007aff', color2: '#22d3ee', labelKey: 'settings.appearance.accentBlue' },
  { id: 'green', color: '#34c759', color2: '#06b6a4', labelKey: 'settings.appearance.accentGreen' },
  {
    id: 'orange',
    color: '#ff9500',
    color2: '#ffb340',
    labelKey: 'settings.appearance.accentOrange',
  },
  { id: 'pink', color: '#ff2d55', color2: '#ff5e9c', labelKey: 'settings.appearance.accentPink' },
  { id: 'red', color: '#ff3b30', color2: '#ff2d7a', labelKey: 'settings.appearance.accentRed' },
  {
    id: 'yellow',
    color: '#ffcc00',
    color2: '#ff9500',
    labelKey: 'settings.appearance.accentYellow',
  },
  {
    id: 'graphite',
    color: '#8e8e93',
    color2: '#6e7280',
    labelKey: 'settings.appearance.accentGraphite',
  },
];
