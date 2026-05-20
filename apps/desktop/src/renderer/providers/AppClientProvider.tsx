import { createContext, type ReactNode, useContext, useEffect, useMemo } from 'react';

import {
  _registerClient,
  type AppClient,
  createDesktopIpcClient,
  getClient,
} from '@/lib/app-client';

export { getClient };

const AppClientContext = createContext<AppClient | null>(null);

export function AppClientProvider({ children }: { children: ReactNode }) {
  const client = useMemo(() => createDesktopIpcClient(), []);

  // Register module-level reference so getClient() works outside React.
  useEffect(() => {
    _registerClient(client);
  }, [client]);

  return <AppClientContext.Provider value={client}>{children}</AppClientContext.Provider>;
}

export function useAppClient(): AppClient {
  const client = useContext(AppClientContext);
  if (!client) throw new Error('useAppClient must be used within <AppClientProvider>');
  return client;
}
