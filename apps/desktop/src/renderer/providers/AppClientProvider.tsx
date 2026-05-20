import { createContext, type ReactNode, useContext, useMemo } from 'react';

import {
  _registerClient,
  type AppClient,
  createDesktopIpcClient,
  getClient,
} from '@/lib/app-client';

export { getClient };

interface AppClientProviderProps {
  children: ReactNode;
  /** Override the transport. Desktop omits this (falls back to Electron IPC).
   *  The Tauri entry and test harnesses pass their own client here. */
  client?: AppClient;
}

const AppClientContext = createContext<AppClient | null>(null);

export function AppClientProvider({ children, client: injected }: AppClientProviderProps) {
  const client = useMemo(() => {
    const c = injected ?? createDesktopIpcClient();
    _registerClient(c);
    return c;
  }, [injected]);

  return <AppClientContext.Provider value={client}>{children}</AppClientContext.Provider>;
}

export function useAppClient(): AppClient {
  const client = useContext(AppClientContext);
  if (!client) throw new Error('useAppClient must be used within <AppClientProvider>');
  return client;
}
