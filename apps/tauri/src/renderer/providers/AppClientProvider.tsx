import { createContext, type ReactNode, useContext, useMemo } from 'react';

import { _registerClient, type AppClient, getClient } from '@/lib/app-client';

export { getClient };

interface AppClientProviderProps {
  children: ReactNode;
  /** Override the transport. Tauri entry passes createTauriInvokeClient(); tests pass a mock. */
  client?: AppClient;
}

const AppClientContext = createContext<AppClient | null>(null);

export function AppClientProvider({ children, client: injected }: AppClientProviderProps) {
  const client = useMemo(() => {
    if (!injected) throw new Error('AppClientProvider requires a client prop');
    _registerClient(injected);
    return injected;
  }, [injected]);

  return <AppClientContext.Provider value={client}>{children}</AppClientContext.Provider>;
}

export function useAppClient(): AppClient {
  const client = useContext(AppClientContext);
  if (!client) throw new Error('useAppClient must be used within <AppClientProvider>');
  return client;
}
