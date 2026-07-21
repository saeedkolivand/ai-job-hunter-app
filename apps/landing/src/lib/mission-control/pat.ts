import { MC_CONFIG } from './config';

// Fine-grained PAT storage for /mission-control. The token lives ONLY in this
// browser's localStorage (surfaced with an explicit banner in the UI) and is
// sent ONLY as an Authorization header to api.github.com (see github.ts).
// Sign-out wipes it. All access is guarded so the module is import-safe under
// SSR / static export (localStorage is only touched inside these functions,
// which are called from client effects and event handlers).

// Scope guidance shown in the sign-in panel — least privilege for the safe tier.
export const PAT_SCOPE_GUIDANCE = [
  { label: 'Repository access', value: 'Only ai-job-hunter-app (this repo)' },
  { label: 'Actions', value: 'Read and write — re-run + dispatch release/pages' },
  { label: 'Issues', value: 'Read and write — close/reopen/label/comment' },
  { label: 'Contents', value: 'Read-only' },
  { label: 'Pull requests', value: 'Read-only (no merge — the dashboard never merges)' },
] as const;

export function readToken(): string {
  try {
    return localStorage.getItem(MC_CONFIG.tokenKey) ?? '';
  } catch {
    return '';
  }
}

export function saveToken(token: string): void {
  try {
    const trimmed = token.trim();
    if (trimmed) localStorage.setItem(MC_CONFIG.tokenKey, trimmed);
    else localStorage.removeItem(MC_CONFIG.tokenKey);
  } catch {
    // storage disabled — nothing to persist
  }
}

// Sign-out: wipe the token from storage entirely.
export function clearToken(): void {
  try {
    localStorage.removeItem(MC_CONFIG.tokenKey);
  } catch {
    // storage disabled — nothing to wipe
  }
}

export function isSignedIn(): boolean {
  return readToken().length > 0;
}
