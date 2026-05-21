import type { ComponentType } from 'react';

let _controls: ComponentType | null = null;

export function registerWindowControls(component: ComponentType) {
  _controls = component;
}

export function getWindowControls(): ComponentType | null {
  return _controls;
}
