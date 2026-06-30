import type { ReactNode } from 'react';

import { ErrorState } from '@ajh/ui';

import { useProtocolVersionCheck } from '@/services/use-system';

/**
 * Blocks the app when the renderer and Rust shell disagree on the IPC contract
 * version. In a single-binary build they always match; a mismatch means a stale
 * webview cache or partial install, where IPC calls can silently misbehave.
 */
export function ProtocolVersionGate({ children }: { children: ReactNode }) {
  const { mismatch, expected, actual } = useProtocolVersionCheck();

  if (mismatch) {
    return (
      <div className="flex h-screen items-center justify-center">
        <ErrorState
          title="App update incomplete"
          description={`This app's interface (v${expected}) does not match its engine (v${actual ?? 'unknown'}). Fully restart the app; if it persists, reinstall the latest version.`}
          onRetry={() => window.location.reload()}
        />
      </div>
    );
  }

  return <>{children}</>;
}
