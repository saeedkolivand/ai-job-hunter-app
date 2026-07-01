import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { waitFor } from '@testing-library/react';

import { PROTOCOL_VERSION } from '@ajh/shared';

import { usePreferencesStore } from '@/store/preferences-store';
import { createMockClient, renderHookWithClient } from '@/test-support';

import {
  useAppVersion,
  useGetPlatform,
  useLaunchAtLogin,
  useProtocolVersionCheck,
  useSetCloseToTray,
  useSetLaunchAtLogin,
  useSetLocale,
  useSyncCloseToTray,
  useSystemHealth,
} from './use-system';

describe('use-system services', () => {
  it('useSystemHealth queries system.health', async () => {
    const client = createMockClient({ 'system.health': vi.fn().mockResolvedValue({ ok: true }) });
    const { result } = renderHookWithClient(() => useSystemHealth(), { client });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toEqual({ ok: true });
  });

  it('useAppVersion returns the backend version', async () => {
    const client = createMockClient({ 'system.getVersion': vi.fn().mockResolvedValue('1.2.3') });
    const { result } = renderHookWithClient(() => useAppVersion(), { client });
    await waitFor(() => expect(result.current.data).toBe('1.2.3'));
  });

  it('useGetPlatform queries the platform', async () => {
    const client = createMockClient({ 'system.getPlatform': vi.fn().mockResolvedValue('win32') });
    const { result } = renderHookWithClient(() => useGetPlatform(), { client });
    await waitFor(() => expect(result.current.data).toBe('win32'));
  });

  it('useProtocolVersionCheck flags a mismatch', async () => {
    const client = createMockClient({
      'system.getProtocolVersion': vi.fn().mockResolvedValue('0.0.0-stale'),
    });
    const { result } = renderHookWithClient(() => useProtocolVersionCheck(), { client });
    await waitFor(() => expect(result.current.checked).toBe(true));
    expect(result.current.mismatch).toBe(true);
    expect(result.current.expected).toBe(PROTOCOL_VERSION);
  });

  it('useProtocolVersionCheck reports a match for the current version', async () => {
    const client = createMockClient({
      'system.getProtocolVersion': vi.fn().mockResolvedValue(PROTOCOL_VERSION),
    });
    const { result } = renderHookWithClient(() => useProtocolVersionCheck(), { client });
    await waitFor(() => expect(result.current.checked).toBe(true));
    expect(result.current.mismatch).toBe(false);
  });

  it('useSetLocale mutation calls system.setLocale', async () => {
    const setLocale = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setLocale': setLocale });
    const { result } = renderHookWithClient(() => useSetLocale(), { client });
    result.current.mutate('de');
    await waitFor(() => expect(setLocale).toHaveBeenCalledWith('de'));
  });

  it('useLaunchAtLogin reads the current OS state', async () => {
    const client = createMockClient({
      'system.getLaunchAtLogin': vi.fn().mockResolvedValue(true),
    });
    const { result } = renderHookWithClient(() => useLaunchAtLogin(), { client });
    await waitFor(() => expect(result.current.data).toBe(true));
  });

  it('useSetLaunchAtLogin toggles via system.setLaunchAtLogin and resolves to the applied state', async () => {
    const setLaunchAtLogin = vi.fn().mockResolvedValue(true);
    const client = createMockClient({ 'system.setLaunchAtLogin': setLaunchAtLogin });
    const { result } = renderHookWithClient(() => useSetLaunchAtLogin(), { client });
    result.current.mutate(true);
    await waitFor(() => expect(setLaunchAtLogin).toHaveBeenCalledWith(true));
    await waitFor(() => expect(result.current.data).toBe(true));
  });
});

describe('close-to-tray sync', () => {
  // Real preferences store (persisted) — reset to defaults before each test so
  // the persisted `closeToTray` value never bleeds across cases.
  beforeEach(() => {
    usePreferencesStore.getState().resetPreferences();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('useSetCloseToTray pushes IPC then persists the preference on success', async () => {
    const setCloseToTray = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setCloseToTray': setCloseToTray });
    // Baseline is the default `true`; a successful toggle must flip the store.
    expect(usePreferencesStore.getState().closeToTray).toBe(true);

    const { result } = renderHookWithClient(() => useSetCloseToTray(), { client });
    result.current.mutate(false);

    await waitFor(() => expect(setCloseToTray).toHaveBeenCalledExactlyOnceWith(false));
    // onSuccess persists the preference (the store is the source of truth).
    await waitFor(() => expect(usePreferencesStore.getState().closeToTray).toBe(false));
  });

  it('useSetCloseToTray does NOT persist when the IPC push rejects (the race fix)', async () => {
    const setCloseToTray = vi.fn().mockRejectedValue(new Error('shell offline'));
    const client = createMockClient({ 'system.setCloseToTray': setCloseToTray });
    // Baseline `true`; a rejected push must NOT flip the persisted preference,
    // so the store and the Rust flag can't diverge.
    expect(usePreferencesStore.getState().closeToTray).toBe(true);

    const { result } = renderHookWithClient(() => useSetCloseToTray(), { client });
    result.current.mutate(false);

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(setCloseToTray).toHaveBeenCalledExactlyOnceWith(false);
    expect(usePreferencesStore.getState().closeToTray).toBe(true);
  });

  it('useSyncCloseToTray pushes the persisted value to the shell exactly once on mount', async () => {
    // Persist a non-default choice so we can assert it (not the Rust default) is pushed.
    usePreferencesStore.getState().setCloseToTray(false);
    const setCloseToTray = vi.fn().mockResolvedValue(undefined);
    const client = createMockClient({ 'system.setCloseToTray': setCloseToTray });

    const { rerender } = renderHookWithClient(() => useSyncCloseToTray(), { client });
    await waitFor(() => expect(setCloseToTray).toHaveBeenCalledExactlyOnceWith(false));

    // A re-render must not re-push — the useRef guard fires the effect body once.
    rerender();
    expect(setCloseToTray).toHaveBeenCalledTimes(1);
  });
});
