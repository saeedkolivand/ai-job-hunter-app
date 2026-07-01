import type { ComponentType } from 'react';

let _controls: ComponentType | null = null;
const _listeners: Array<(c: ComponentType) => void> = [];

export function registerWindowControls(component: ComponentType) {
  _controls = component;
  _listeners.forEach((fn) => fn(component));
  _listeners.length = 0;
}

export function getWindowControls(): ComponentType | null {
  return _controls;
}

export function onWindowControlsRegistered(fn: (c: ComponentType) => void) {
  if (_controls) fn(_controls);
  else _listeners.push(fn);
}
