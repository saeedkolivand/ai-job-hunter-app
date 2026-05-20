import { createContext, type ReactNode, useContext, useMemo } from 'react';

import {
  _registerClient,
  type AppClient,
  createDesktopIpcClient,
  getClient,
} from '@/lib/app-client';

export { getClient };

const AppClientContext = createContext<AppClient | null>(null);

export function AppClientProvider({ children }: { children: ReactNode }) {
  const client = useMemo(() => {
    const c = createDesktopIpcClient();
    _registerClient(c);
    return c;
  }, []);

  return <AppClientContext.Provider value={client}>{children}</AppClientContext.Provider>;
}

export function useAppClient(): AppClient {
  const client = useContext(AppClientContext);
  if (!client) throw new Error('useAppClient must be used within <AppClientProvider>');
  return client;
}
