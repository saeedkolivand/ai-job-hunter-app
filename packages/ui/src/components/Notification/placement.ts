import type { CSSProperties } from 'react';

import type { NotificationPlacement } from './types';

export const PLACEMENTS: NotificationPlacement[] = [
  'top',
  'topLeft',
  'topRight',
  'bottom',
  'bottomLeft',
  'bottomRight',
];

/** Fixed-container anchor for a placement. */
export function containerStyle(placement: NotificationPlacement): CSSProperties {
  const base: CSSProperties = {
    position: 'fixed',
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
    pointerEvents: 'none',
    zIndex: 2147483647,
    maxWidth: 'calc(100vw - 48px)',
  };
  const top = placement.startsWith('top');
  const vertical: CSSProperties = top ? { top: '24px' } : { bottom: '24px' };
  let horizontal: CSSProperties;
  let align: CSSProperties['alignItems'];
  if (placement.endsWith('Left')) {
    horizontal = { left: '24px' };
    align = 'flex-start';
  } else if (placement.endsWith('Right')) {
    horizontal = { right: '24px' };
    align = 'flex-end';
  } else {
    horizontal = { left: '50%', transform: 'translateX(-50%)' };
    align = 'center';
  }
  return { ...base, ...vertical, ...horizontal, alignItems: align };
}

/** Enter/exit offset so each placement slides in from its own edge. */
export function slideOffset(placement: NotificationPlacement): { x: number; y: number } {
  if (placement.endsWith('Right')) return { x: 40, y: 0 };
  if (placement.endsWith('Left')) return { x: -40, y: 0 };
  return { x: 0, y: placement.startsWith('top') ? -40 : 40 };
}
