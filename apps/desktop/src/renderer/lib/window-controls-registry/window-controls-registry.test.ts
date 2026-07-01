import { beforeEach, describe, expect, it, vi } from 'vitest';

// The registry holds module-level state; reset the module between tests.
beforeEach(() => {
  vi.resetModules();
});

async function load() {
  return import('./window-controls-registry');
}

const Controls = () => null;

describe('window-controls-registry', () => {
  it('returns null before any controls are registered', async () => {
    const { getWindowControls } = await load();
    expect(getWindowControls()).toBeNull();
  });

  it('stores the registered component', async () => {
    const { registerWindowControls, getWindowControls } = await load();
    registerWindowControls(Controls);
    expect(getWindowControls()).toBe(Controls);
  });

  it('notifies subscribers registered before the component', async () => {
    const { onWindowControlsRegistered, registerWindowControls } = await load();
    const fn = vi.fn();
    onWindowControlsRegistered(fn);
    expect(fn).not.toHaveBeenCalled();
    registerWindowControls(Controls);
    expect(fn).toHaveBeenCalledWith(Controls);
  });

  it('invokes a late subscriber immediately with the current component', async () => {
    const { onWindowControlsRegistered, registerWindowControls } = await load();
    registerWindowControls(Controls);
    const fn = vi.fn();
    onWindowControlsRegistered(fn);
    expect(fn).toHaveBeenCalledWith(Controls);
  });
});
